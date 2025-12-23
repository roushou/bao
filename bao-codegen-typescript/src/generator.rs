//! TypeScript code generator using boune framework.

use std::{collections::HashSet, path::Path};

use baobao_codegen::{
    AppIR,
    generation::{FileEntry, FileRegistry, HandlerPaths, find_orphan_commands},
    language::{CleanResult, GenerateResult, LanguageCodegen, PreviewFile},
    pipeline::CompilationContext,
    schema::{CommandInfo, ComputedData},
};
use baobao_core::{GeneratedFile, to_camel_case, to_kebab_case, to_pascal_case};
use baobao_ir::{CommandOp, InputKind, Operation};
use eyre::Result;

use crate::{
    adapters::BouneAdapter,
    ast::{Import, JsObject},
    files::{
        CliTs, CommandTs, ContextTs, GitIgnore, HandlerTs, IndexTs, PackageJson, STUB_MARKER,
        TsConfig,
    },
};

/// TypeScript code generator that produces boune-based CLI code for Bun.
pub struct Generator {
    ir: AppIR,
    computed: ComputedData,
    cli_adapter: BouneAdapter,
}

impl LanguageCodegen for Generator {
    fn language(&self) -> &'static str {
        "typescript"
    }

    fn file_extension(&self) -> &'static str {
        "ts"
    }

    fn preview(&self) -> Vec<PreviewFile> {
        self.preview_files()
    }

    fn generate(&self, output_dir: &Path) -> Result<GenerateResult> {
        self.generate_files(output_dir)
    }

    fn clean(&self, output_dir: &Path) -> Result<CleanResult> {
        self.clean_files(output_dir)
    }

    fn preview_clean(&self, output_dir: &Path) -> Result<CleanResult> {
        self.preview_clean_files(output_dir)
    }
}

impl Generator {
    /// Create a generator from a compilation context.
    ///
    /// Use `Pipeline::run()` to create the context, then pass it here.
    ///
    /// # Panics
    ///
    /// Panics if the context doesn't have IR or computed data
    /// (i.e., if the pipeline didn't run successfully).
    pub fn from_context(mut ctx: CompilationContext) -> Self {
        Self {
            ir: ctx.take_ir(),
            computed: ctx.take_computed(),
            cli_adapter: BouneAdapter::new(),
        }
    }

    /// Build a file registry with all generated files.
    ///
    /// This centralizes file registration, making generation declarative.
    fn build_registry(&self) -> FileRegistry {
        let mut registry = FileRegistry::new();

        // Use pre-computed data from pipeline
        let context_fields = self.computed.context_fields.clone();

        // Config files
        registry.register(FileEntry::config(
            "package.json",
            PackageJson::new(&self.ir.meta.name)
                .with_version_str(&self.ir.meta.version)
                .render(),
        ));
        registry.register(FileEntry::config("tsconfig.json", TsConfig.render()));
        registry.register(FileEntry::config(".gitignore", GitIgnore.render()));

        // Infrastructure files
        registry.register(FileEntry::infrastructure("src/index.ts", IndexTs.render()));
        registry.register(FileEntry::infrastructure(
            "src/context.ts",
            ContextTs::new(context_fields).render(),
        ));

        // Collect commands from IR
        let commands: Vec<CommandInfo> = self
            .ir
            .operations
            .iter()
            .map(|op| {
                let Operation::Command(cmd) = op;
                CommandInfo {
                    name: cmd.name.clone(),
                    description: cmd.description.clone(),
                    has_subcommands: cmd.has_subcommands(),
                }
            })
            .collect();

        registry.register(FileEntry::generated(
            "src/cli.ts",
            CliTs::new(
                &self.ir.meta.name,
                &self.ir.meta.version,
                self.ir.meta.description.clone(),
                commands,
            )
            .render(),
        ));

        // Individual command files from IR (recursively collect all commands)
        for op in &self.ir.operations {
            let Operation::Command(cmd) = op;
            self.register_command_files_from_ir(&mut registry, cmd);
        }

        registry
    }

    /// Recursively register command files from IR.
    fn register_command_files_from_ir(&self, registry: &mut FileRegistry, cmd: &CommandOp) {
        let content = self.generate_command_file_from_ir(cmd);
        let file_path = cmd
            .path
            .iter()
            .map(|s| to_kebab_case(s))
            .collect::<Vec<_>>()
            .join("/");

        registry.register(FileEntry::generated(
            format!("src/commands/{}.ts", file_path),
            CommandTs::nested(cmd.path.clone(), content).render(),
        ));

        // Recursively register subcommand files
        for child in &cmd.children {
            self.register_command_files_from_ir(registry, child);
        }
    }

    /// Preview generated files without writing to disk.
    fn preview_files(&self) -> Vec<PreviewFile> {
        self.build_registry()
            .preview()
            .into_iter()
            .map(|entry| PreviewFile {
                path: entry.path,
                content: entry.content,
            })
            .collect()
    }

    /// Generate all files into the specified output directory.
    fn generate_files(&self, output_dir: &Path) -> Result<GenerateResult> {
        let handlers_dir = output_dir.join("src/handlers");

        // Write all registered files using the registry
        let registry = self.build_registry();
        registry.write_all(output_dir)?;

        // Generate handlers (handled separately due to special logic)
        let result = self.generate_handlers(&handlers_dir, output_dir)?;

        Ok(result)
    }

    // ========================================================================
    // IR-based command generation methods
    // ========================================================================

    /// Generate a command file from IR CommandOp.
    fn generate_command_file_from_ir(&self, cmd: &CommandOp) -> String {
        if cmd.has_subcommands() {
            self.generate_parent_command_file_from_ir(cmd)
        } else {
            self.generate_leaf_command_file_from_ir(cmd)
        }
    }

    /// Generate a parent command file from IR.
    fn generate_parent_command_file_from_ir(&self, cmd: &CommandOp) -> String {
        use crate::code_file::{CodeFile, RawCode};

        let camel_name = to_camel_case(&cmd.name);
        let kebab_name = to_kebab_case(&cmd.name);

        // Build imports
        let mut imports = vec![Import::new("boune").named("defineCommand")];
        for child in &cmd.children {
            let sub_camel = to_camel_case(&child.name);
            let sub_kebab = to_kebab_case(&child.name);
            imports.push(
                Import::new(format!("./{}/{}.ts", kebab_name, sub_kebab))
                    .named(format!("{}Command", sub_camel)),
            );
        }

        // Build subcommands object
        let subcommands = cmd.children.iter().fold(JsObject::new(), |obj, child| {
            let sub_camel = to_camel_case(&child.name);
            obj.raw(&sub_camel, format!("{}Command", sub_camel))
        });

        // Build command schema
        let schema = JsObject::new()
            .string("name", &cmd.name)
            .string("description", &cmd.description)
            .object("subcommands", subcommands);

        // Build the command definition string
        let schema_obj = schema.build();
        let command_def = format!(
            "export const {}Command = defineCommand({});",
            camel_name,
            schema_obj.trim_end()
        );

        CodeFile::new()
            .imports(imports)
            .add(RawCode::new(command_def))
            .render()
    }

    /// Generate a leaf command file from IR.
    fn generate_leaf_command_file_from_ir(&self, cmd: &CommandOp) -> String {
        use crate::code_file::{CodeFile, RawCode};

        let camel_name = to_camel_case(&cmd.name);
        let pascal_name = to_pascal_case(&cmd.name);

        // Build the handler path (kebab-case, joined by /)
        let handler_path = cmd
            .path
            .iter()
            .map(|s| to_kebab_case(s))
            .collect::<Vec<_>>()
            .join("/");

        // Calculate relative path from command location
        let depth = cmd.path.len();
        let up_path = "../".repeat(depth);

        // Check for args (positional) and options (flags)
        let has_args = cmd
            .inputs
            .iter()
            .any(|i| matches!(i.kind, InputKind::Positional));
        let has_options = cmd
            .inputs
            .iter()
            .any(|i| matches!(i.kind, InputKind::Flag { .. }));

        // Build imports
        let mut boune_import = Import::new("boune").named("defineCommand");
        if has_args {
            boune_import = boune_import.named_type("InferArgs");
        }
        if has_options {
            boune_import = boune_import.named_type("InferOpts");
        }

        let imports = vec![
            boune_import,
            Import::new(format!("{}handlers/{}.ts", up_path, handler_path)).named("run"),
        ];

        // Build body parts
        let mut body_parts: Vec<String> = Vec::new();

        // Arguments schema as const
        if has_args {
            let arguments = cmd
                .inputs
                .iter()
                .filter(|i| matches!(i.kind, InputKind::Positional))
                .fold(JsObject::new(), |obj, input| {
                    let camel = to_camel_case(&input.name);
                    obj.object(&camel, self.build_argument_schema_from_ir(input))
                });

            let args_obj = arguments.build();
            body_parts.push(format!("const args = {} as const;", args_obj.trim_end()));
        }

        // Options schema as const
        if has_options {
            let options = cmd
                .inputs
                .iter()
                .filter(|i| matches!(i.kind, InputKind::Flag { .. }))
                .fold(JsObject::new(), |obj, input| {
                    let camel = to_camel_case(&input.name);
                    obj.object(&camel, self.build_option_schema_from_ir(input))
                });

            let opts_obj = options.build();
            body_parts.push(format!("const options = {} as const;", opts_obj.trim_end()));
        }

        // Command definition
        let command_def =
            self.build_command_definition_from_ir(&camel_name, cmd, has_args, has_options);
        body_parts.push(command_def);

        // Export inferred types
        let mut type_exports = Vec::new();
        if has_args {
            type_exports.push(format!(
                "export type {}Args = InferArgs<typeof args>;",
                pascal_name
            ));
        }
        if has_options {
            type_exports.push(format!(
                "export type {}Options = InferOpts<typeof options>;",
                pascal_name
            ));
        }
        if !type_exports.is_empty() {
            body_parts.push(type_exports.join("\n"));
        }

        let mut file = CodeFile::new().imports(imports);
        for part in body_parts {
            file = file.add(RawCode::new(part));
        }
        file.render()
    }

    fn build_command_definition_from_ir(
        &self,
        camel_name: &str,
        cmd: &CommandOp,
        has_args: bool,
        has_options: bool,
    ) -> String {
        // Build action handler body
        let action = self.cli_adapter.build_action_handler(has_args, has_options);

        // Build command schema - reference extracted consts
        let schema = JsObject::new()
            .string("name", &cmd.name)
            .string("description", &cmd.description)
            .raw_if(has_args, "arguments", "args")
            .raw_if(has_options, "options", "options")
            .arrow_fn("action", action);

        let schema_obj = schema.build();
        format!(
            "export const {}Command = defineCommand({});",
            camel_name,
            schema_obj.trim_end()
        )
    }

    fn build_argument_schema_from_ir(&self, input: &baobao_ir::Input) -> JsObject {
        self.cli_adapter.build_argument_schema_ir(input)
    }

    fn build_option_schema_from_ir(&self, input: &baobao_ir::Input) -> JsObject {
        self.cli_adapter.build_option_schema_ir(input)
    }

    /// Generate handlers directory with stub files for missing handlers.
    fn generate_handlers(&self, handlers_dir: &Path, _output_dir: &Path) -> Result<GenerateResult> {
        let mut created_handlers = Vec::new();

        // Collect all expected handler paths from computed data (kebab-case for TypeScript file names)
        let expected_handlers: HashSet<String> = self
            .computed
            .command_paths
            .iter()
            .map(|path| {
                path.split('/')
                    .map(to_kebab_case)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .collect();

        // Ensure handlers directory exists
        std::fs::create_dir_all(handlers_dir)?;

        // Process commands recursively from IR
        for op in &self.ir.operations {
            let Operation::Command(cmd) = op;
            self.generate_handlers_for_command(cmd, handlers_dir, &mut created_handlers)?;
        }

        // Find orphan handlers using shared utility
        let handler_paths = HandlerPaths::new(handlers_dir, "ts", STUB_MARKER);
        let orphan_handlers = handler_paths.find_orphans(&expected_handlers)?;

        Ok(GenerateResult {
            created_handlers,
            orphan_handlers,
        })
    }

    /// Recursively generate handlers for a command and its children.
    fn generate_handlers_for_command(
        &self,
        cmd: &CommandOp,
        handlers_dir: &Path,
        created_handlers: &mut Vec<String>,
    ) -> Result<()> {
        use baobao_core::WriteResult;

        let handler_path: Vec<&str> = cmd.path.iter().map(|s| s.as_str()).collect();
        let dir = handler_path
            .iter()
            .take(handler_path.len().saturating_sub(1))
            .fold(handlers_dir.to_path_buf(), |acc, segment| {
                acc.join(to_kebab_case(segment))
            });

        if cmd.has_subcommands() {
            // Parent command - create directory
            let cmd_dir = dir.join(to_kebab_case(&cmd.name));
            std::fs::create_dir_all(&cmd_dir)?;

            // Recursively process children
            for child in &cmd.children {
                self.generate_handlers_for_command(child, handlers_dir, created_handlers)?;
            }
        } else {
            // Leaf command - create handler stub
            std::fs::create_dir_all(&dir)?;

            let display_path = cmd
                .path
                .iter()
                .map(|s| to_kebab_case(s))
                .collect::<Vec<_>>()
                .join("/");
            let path_segments = cmd.path.clone();
            let has_args = cmd
                .inputs
                .iter()
                .any(|i| matches!(i.kind, InputKind::Positional));
            let has_options = cmd
                .inputs
                .iter()
                .any(|i| matches!(i.kind, InputKind::Flag { .. }));

            let stub = HandlerTs::nested(&cmd.name, path_segments, has_args, has_options);
            let result = stub.write(&dir)?;

            if matches!(result, WriteResult::Written) {
                created_handlers.push(format!("{}.ts", display_path));
            }
        }

        Ok(())
    }

    /// Clean orphaned generated files.
    fn clean_files(&self, output_dir: &Path) -> Result<CleanResult> {
        let mut result = CleanResult::default();

        // Collect expected command names from IR (kebab-case for file names)
        let expected_commands: HashSet<String> = self
            .ir
            .operations
            .iter()
            .map(|op| {
                let Operation::Command(cmd) = op;
                to_kebab_case(&cmd.name)
            })
            .collect();

        // Collect expected handler paths from computed data (kebab-case)
        let expected_handlers: HashSet<String> = self
            .computed
            .command_paths
            .iter()
            .map(|path| {
                path.split('/')
                    .map(to_kebab_case)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .collect();

        // Find and delete orphaned generated command files
        let commands_dir = output_dir.join("src/commands");
        let orphan_commands = find_orphan_commands(&commands_dir, "ts", &expected_commands)?;
        for path in orphan_commands {
            std::fs::remove_file(&path)?;
            let relative = path.strip_prefix(output_dir).unwrap_or(&path);
            result.deleted_commands.push(relative.display().to_string());
        }

        // Find and handle orphaned handler files
        let handlers_dir = output_dir.join("src/handlers");
        let handler_paths = HandlerPaths::new(&handlers_dir, "ts", STUB_MARKER);
        let orphan_handlers = handler_paths.find_orphans_with_status(&expected_handlers)?;

        for orphan in orphan_handlers {
            if orphan.is_unmodified {
                // Safe to delete - it's still just a stub
                std::fs::remove_file(&orphan.full_path)?;
                result
                    .deleted_handlers
                    .push(format!("src/handlers/{}.ts", orphan.relative_path));

                // Try to clean up empty parent directories
                if let Some(parent) = orphan.full_path.parent() {
                    let _ = Self::remove_empty_dirs(parent, &handlers_dir);
                }
            } else {
                // User has modified this file, skip it
                result
                    .skipped_handlers
                    .push(format!("src/handlers/{}.ts", orphan.relative_path));
            }
        }

        Ok(result)
    }

    /// Preview what would be cleaned without actually deleting files.
    fn preview_clean_files(&self, output_dir: &Path) -> Result<CleanResult> {
        let mut result = CleanResult::default();

        // Collect expected command names from IR (kebab-case for file names)
        let expected_commands: HashSet<String> = self
            .ir
            .operations
            .iter()
            .map(|op| {
                let Operation::Command(cmd) = op;
                to_kebab_case(&cmd.name)
            })
            .collect();

        // Collect expected handler paths from computed data (kebab-case)
        let expected_handlers: HashSet<String> = self
            .computed
            .command_paths
            .iter()
            .map(|path| {
                path.split('/')
                    .map(to_kebab_case)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .collect();

        // Find orphaned generated command files
        let commands_dir = output_dir.join("src/commands");
        let orphan_commands = find_orphan_commands(&commands_dir, "ts", &expected_commands)?;
        for path in orphan_commands {
            let relative = path.strip_prefix(output_dir).unwrap_or(&path);
            result.deleted_commands.push(relative.display().to_string());
        }

        // Find orphaned handler files
        let handlers_dir = output_dir.join("src/handlers");
        let handler_paths = HandlerPaths::new(&handlers_dir, "ts", STUB_MARKER);
        let orphan_handlers = handler_paths.find_orphans_with_status(&expected_handlers)?;

        for orphan in orphan_handlers {
            if orphan.is_unmodified {
                result
                    .deleted_handlers
                    .push(format!("src/handlers/{}.ts", orphan.relative_path));
            } else {
                result
                    .skipped_handlers
                    .push(format!("src/handlers/{}.ts", orphan.relative_path));
            }
        }

        Ok(result)
    }

    /// Remove empty directories up to but not including the base directory.
    fn remove_empty_dirs(dir: &Path, base: &Path) -> Result<()> {
        if dir == base || !dir.starts_with(base) {
            return Ok(());
        }

        // Check if directory is empty
        if std::fs::read_dir(dir)?.next().is_none() {
            std::fs::remove_dir(dir)?;
            // Try to remove parent too
            if let Some(parent) = dir.parent() {
                let _ = Self::remove_empty_dirs(parent, base);
            }
        }

        Ok(())
    }
}
