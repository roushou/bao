//! TypeScript code generator using boune framework.

use std::{collections::HashSet, path::Path};

use baobao_codegen::{
    builder::CodeBuilder,
    generation::{FileEntry, FileRegistry, HandlerPaths, find_orphan_commands},
    language::{CleanResult, GenerateResult, LanguageCodegen, PreviewFile},
    schema::{CommandInfo, CommandTree, collect_context_fields},
};
use baobao_core::{GeneratedFile, to_camel_case, to_kebab_case};
use baobao_manifest::{Command, Manifest};
use eyre::Result;

use crate::{
    adapters::BouneAdapter,
    ast::{Import, JsObject},
    files::{CliTs, CommandTs, ContextTs, GitIgnore, HandlerTs, IndexTs, PackageJson, TsConfig},
};

/// Marker string indicating an unmodified TypeScript handler stub.
const STUB_MARKER: &str = "// TODO: implement";

/// TypeScript code generator that produces boune-based CLI code for Bun.
pub struct Generator<'a> {
    schema: &'a Manifest,
    cli_adapter: BouneAdapter,
}

impl LanguageCodegen for Generator<'_> {
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

impl<'a> Generator<'a> {
    pub fn new(schema: &'a Manifest) -> Self {
        Self {
            schema,
            cli_adapter: BouneAdapter::new(),
        }
    }

    /// Build a file registry with all generated files.
    ///
    /// This centralizes file registration, making generation declarative.
    fn build_registry(&self) -> FileRegistry {
        let mut registry = FileRegistry::new();

        // Collect context field info
        let context_fields = collect_context_fields(&self.schema.context);

        // Config files
        registry.register(FileEntry::config(
            "package.json",
            PackageJson::new(&self.schema.cli.name)
                .with_version(self.schema.cli.version.clone())
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

        // Generated files
        let commands: Vec<CommandInfo> = self
            .schema
            .commands
            .iter()
            .map(|(name, cmd)| CommandInfo {
                name: name.clone(),
                description: cmd.description.clone(),
                has_subcommands: cmd.has_subcommands(),
            })
            .collect();

        registry.register(FileEntry::generated(
            "src/cli.ts",
            CliTs::new(
                &self.schema.cli.name,
                self.schema.cli.version.clone(),
                self.schema.cli.description.clone(),
                commands,
            )
            .render(),
        ));

        // Individual command files (recursively collect all commands)
        for (name, command) in &self.schema.commands {
            self.register_command_files(&mut registry, name, command, vec![name.clone()]);
        }

        registry
    }

    /// Recursively register command files in the registry.
    fn register_command_files(
        &self,
        registry: &mut FileRegistry,
        name: &str,
        command: &Command,
        path_segments: Vec<String>,
    ) {
        let content = self.generate_command_file(name, command, &path_segments);
        let file_path = path_segments
            .iter()
            .map(|s| to_kebab_case(s))
            .collect::<Vec<_>>()
            .join("/");

        registry.register(FileEntry::generated(
            format!("src/commands/{}.ts", file_path),
            CommandTs::nested(path_segments.clone(), content).render(),
        ));

        // Recursively register subcommand files
        if command.has_subcommands() {
            for (sub_name, sub_command) in &command.commands {
                let mut sub_path = path_segments.clone();
                sub_path.push(sub_name.clone());
                self.register_command_files(registry, sub_name, sub_command, sub_path);
            }
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

    fn generate_command_file(
        &self,
        name: &str,
        command: &Command,
        path_segments: &[String],
    ) -> String {
        if command.has_subcommands() {
            self.generate_parent_command_file(name, command)
        } else {
            self.generate_leaf_command_file(name, command, path_segments)
        }
    }

    /// Generate a parent command file that only routes to subcommands.
    fn generate_parent_command_file(&self, name: &str, command: &Command) -> String {
        use crate::code_file::{CodeFile, RawCode};

        let camel_name = to_camel_case(name);
        let kebab_name = to_kebab_case(name);

        // Build imports
        let mut imports = vec![Import::new("boune").named("defineCommand")];
        for subcommand_name in command.commands.keys() {
            let sub_camel = to_camel_case(subcommand_name);
            let sub_kebab = to_kebab_case(subcommand_name);
            imports.push(
                Import::new(format!("./{}/{}.ts", kebab_name, sub_kebab))
                    .named(format!("{}Command", sub_camel)),
            );
        }

        // Build subcommands object
        let subcommands = command
            .commands
            .keys()
            .fold(JsObject::new(), |obj, sub_name| {
                let sub_camel = to_camel_case(sub_name);
                obj.raw(&sub_camel, format!("{}Command", sub_camel))
            });

        // Build command schema
        let schema = JsObject::new()
            .string("name", name)
            .string("description", &command.description)
            .object("subcommands", subcommands);

        // Build the command definition string
        let mut builder = CodeBuilder::typescript();
        builder
            .push_raw(&format!(
                "export const {}Command = defineCommand(",
                camel_name
            ))
            .emit(&schema)
            .push_raw(");");
        let command_def = builder.build();

        CodeFile::new()
            .imports(imports)
            .add(RawCode::new(command_def))
            .render()
    }

    /// Generate a leaf command file that has an action handler.
    fn generate_leaf_command_file(
        &self,
        name: &str,
        command: &Command,
        path_segments: &[String],
    ) -> String {
        use baobao_core::to_pascal_case;

        use crate::code_file::{CodeFile, RawCode};

        let camel_name = to_camel_case(name);
        let pascal_name = to_pascal_case(name);

        // Build the handler path (kebab-case, joined by /)
        let handler_path = path_segments
            .iter()
            .map(|s| to_kebab_case(s))
            .collect::<Vec<_>>()
            .join("/");

        // Calculate relative path from command location
        let depth = path_segments.len();
        let up_path = "../".repeat(depth);

        // Build imports
        let mut boune_import = Import::new("boune").named("defineCommand");
        if !command.args.is_empty() {
            boune_import = boune_import.named("argument").named_type("InferArgs");
        }
        if !command.flags.is_empty() {
            boune_import = boune_import.named("option").named_type("InferOptions");
        }

        let imports = vec![
            boune_import,
            Import::new(format!("{}handlers/{}.ts", up_path, handler_path)).named("run"),
        ];

        // Build body parts
        let mut body_parts: Vec<String> = Vec::new();

        // Arguments schema as const
        if !command.args.is_empty() {
            let arguments = command
                .args
                .iter()
                .fold(JsObject::new(), |obj, (arg_name, arg)| {
                    let camel = to_camel_case(arg_name);
                    obj.raw(&camel, self.build_argument_chain(arg))
                });

            let mut builder = CodeBuilder::typescript();
            builder
                .push_raw("const args = ")
                .emit(&arguments)
                .push_line(" as const;");
            body_parts.push(builder.build().trim_end().to_string());
        }

        // Options schema as const
        if !command.flags.is_empty() {
            let options = command
                .flags
                .iter()
                .fold(JsObject::new(), |obj, (flag_name, flag)| {
                    let camel = to_camel_case(flag_name);
                    obj.raw(&camel, self.build_option_chain(flag))
                });

            let mut builder = CodeBuilder::typescript();
            builder
                .push_raw("const options = ")
                .emit(&options)
                .push_line(" as const;");
            body_parts.push(builder.build().trim_end().to_string());
        }

        // Command definition
        let command_def = self.build_command_definition(&camel_name, name, command);
        body_parts.push(command_def);

        // Export inferred types
        let mut type_exports = Vec::new();
        if !command.args.is_empty() {
            type_exports.push(format!(
                "export type {}Args = InferArgs<typeof args>;",
                pascal_name
            ));
        }
        if !command.flags.is_empty() {
            type_exports.push(format!(
                "export type {}Options = InferOptions<typeof options>;",
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

    fn build_command_definition(&self, camel_name: &str, name: &str, command: &Command) -> String {
        // Build action handler body
        let action = self.build_action_handler(command);

        // Build command schema - reference extracted consts
        let schema = JsObject::new()
            .string("name", name)
            .string("description", &command.description)
            .raw_if(!command.args.is_empty(), "arguments", "args")
            .raw_if(!command.flags.is_empty(), "options", "options")
            .arrow_fn("action", action);

        let mut builder = CodeBuilder::typescript();
        builder
            .push_raw(&format!(
                "export const {}Command = defineCommand(",
                camel_name
            ))
            .emit(&schema)
            .push_line(");");
        builder.build().trim_end().to_string()
    }

    fn build_argument_chain(&self, arg: &baobao_manifest::Arg) -> String {
        self.cli_adapter.build_argument_chain_manifest(
            &arg.arg_type,
            arg.required,
            arg.default.is_some(),
            arg.default.as_ref(),
            arg.description.as_deref(),
        )
    }

    fn build_option_chain(&self, flag: &baobao_manifest::Flag) -> String {
        self.cli_adapter.build_option_chain_manifest(
            &flag.flag_type,
            flag.short_char(),
            flag.default.as_ref(),
            flag.description.as_deref(),
        )
    }

    fn build_action_handler(&self, command: &Command) -> crate::ast::ArrowFn {
        self.cli_adapter
            .build_action_handler(!command.args.is_empty(), !command.flags.is_empty())
    }

    /// Generate handlers directory with stub files for missing handlers.
    fn generate_handlers(&self, handlers_dir: &Path, _output_dir: &Path) -> Result<GenerateResult> {
        use baobao_core::WriteResult;

        let mut created_handlers = Vec::new();
        let tree = CommandTree::new(self.schema);

        // Collect all expected handler paths (kebab-case for TypeScript file names)
        let expected_handlers: HashSet<String> = tree
            .iter()
            .map(|cmd| cmd.path_transformed("/", to_kebab_case))
            .collect();

        // Ensure handlers directory exists
        std::fs::create_dir_all(handlers_dir)?;

        // Create directories for all parent commands (command groups)
        for cmd in tree.parents() {
            let dir = cmd.handler_dir(handlers_dir, to_kebab_case);
            std::fs::create_dir_all(&dir)?;
        }

        // Create stub files for all leaf commands (actual handlers)
        for cmd in tree.leaves() {
            let dir = cmd.handler_dir(handlers_dir, to_kebab_case);
            std::fs::create_dir_all(&dir)?;

            let display_path = cmd.path_transformed("/", to_kebab_case);
            let path_segments = cmd.path.iter().map(|s| s.to_string()).collect();
            let has_args = !cmd.command.args.is_empty();
            let has_options = !cmd.command.flags.is_empty();

            let stub = HandlerTs::nested(cmd.name, path_segments, has_args, has_options);
            let result = stub.write(&dir)?;

            if matches!(result, WriteResult::Written) {
                created_handlers.push(format!("{}.ts", display_path));
            }
        }

        // Find orphan handlers using shared utility
        let handler_paths = HandlerPaths::new(handlers_dir, "ts", STUB_MARKER);
        let orphan_handlers = handler_paths.find_orphans(&expected_handlers)?;

        Ok(GenerateResult {
            created_handlers,
            orphan_handlers,
        })
    }

    /// Clean orphaned generated files.
    fn clean_files(&self, output_dir: &Path) -> Result<CleanResult> {
        let mut result = CleanResult::default();

        // Collect expected command names (kebab-case for file names)
        let expected_commands: HashSet<String> = self
            .schema
            .commands
            .keys()
            .map(|name| to_kebab_case(name))
            .collect();

        // Collect expected handler paths (kebab-case)
        let expected_handlers: HashSet<String> = CommandTree::new(self.schema)
            .collect_paths()
            .into_iter()
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

        // Collect expected command names (kebab-case for file names)
        let expected_commands: HashSet<String> = self
            .schema
            .commands
            .keys()
            .map(|name| to_kebab_case(name))
            .collect();

        // Collect expected handler paths (kebab-case)
        let expected_handlers: HashSet<String> = CommandTree::new(self.schema)
            .collect_paths()
            .into_iter()
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
