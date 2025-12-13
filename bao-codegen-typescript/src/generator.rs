//! TypeScript code generator using boune framework.

use std::{collections::HashSet, path::Path};

use baobao_codegen::{
    builder::CodeBuilder,
    generation::HandlerPaths,
    language::{GenerateResult, LanguageCodegen, PreviewFile},
    schema::{CommandInfo, CommandTree, ContextFieldInfo, PoolConfigInfo, SqliteConfigInfo},
};
use baobao_core::{ContextFieldType, DatabaseType, GeneratedFile, to_camel_case, to_kebab_case};
use baobao_manifest::{Command, Language, Manifest};
use eyre::Result;

use crate::{
    adapters::BouneAdapter,
    ast::{Import, JsObject},
    files::{
        BaoToml, CliTs, CommandTs, ContextTs, GitIgnore, HandlerTs, IndexTs, PackageJson, TsConfig,
    },
};

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
}

impl<'a> Generator<'a> {
    pub fn new(schema: &'a Manifest) -> Self {
        Self {
            schema,
            cli_adapter: BouneAdapter::new(),
        }
    }

    /// Preview generated files without writing to disk.
    fn preview_files(&self) -> Vec<PreviewFile> {
        let mut files = Vec::new();

        // Collect context field info
        let context_fields = self.collect_context_fields();

        // context.ts
        files.push(PreviewFile {
            path: "src/context.ts".to_string(),
            content: ContextTs::new(context_fields).render(),
        });

        // index.ts
        files.push(PreviewFile {
            path: "src/index.ts".to_string(),
            content: IndexTs.render(),
        });

        // package.json
        files.push(PreviewFile {
            path: "package.json".to_string(),
            content: PackageJson::new(&self.schema.cli.name)
                .with_version(self.schema.cli.version.clone())
                .render(),
        });

        // tsconfig.json
        files.push(PreviewFile {
            path: "tsconfig.json".to_string(),
            content: TsConfig.render(),
        });

        // .gitignore
        files.push(PreviewFile {
            path: ".gitignore".to_string(),
            content: GitIgnore.render(),
        });

        // cli.ts
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

        files.push(PreviewFile {
            path: "src/cli.ts".to_string(),
            content: CliTs::new(
                &self.schema.cli.name,
                self.schema.cli.version.clone(),
                self.schema.cli.description.clone(),
                commands,
            )
            .render(),
        });

        // Individual command files (recursively collect all commands)
        for (name, command) in &self.schema.commands {
            self.collect_command_previews(&mut files, name, command, vec![name.clone()]);
        }

        files
    }

    /// Recursively collect command file previews.
    fn collect_command_previews(
        &self,
        files: &mut Vec<PreviewFile>,
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

        files.push(PreviewFile {
            path: format!("src/commands/{}.ts", file_path),
            content: CommandTs::nested(path_segments.clone(), content).render(),
        });

        // Recursively collect subcommand previews
        if command.has_subcommands() {
            for (sub_name, sub_command) in &command.commands {
                let mut sub_path = path_segments.clone();
                sub_path.push(sub_name.clone());
                self.collect_command_previews(files, sub_name, sub_command, sub_path);
            }
        }
    }

    /// Generate all files into the specified output directory.
    fn generate_files(&self, output_dir: &Path) -> Result<GenerateResult> {
        let handlers_dir = output_dir.join("src").join("handlers");

        // Collect context field info
        let context_fields = self.collect_context_fields();

        // Generate context.ts
        ContextTs::new(context_fields).write(output_dir)?;

        // Generate index.ts
        IndexTs.write(output_dir)?;

        // Generate package.json
        PackageJson::new(&self.schema.cli.name)
            .with_version(self.schema.cli.version.clone())
            .write(output_dir)?;

        // Generate tsconfig.json
        TsConfig.write(output_dir)?;

        // Generate .gitignore
        GitIgnore.write(output_dir)?;

        // Generate bao.toml
        BaoToml::new(&self.schema.cli.name, Language::TypeScript)
            .with_version(self.schema.cli.version.clone())
            .write(output_dir)?;

        // Generate cli.ts with main CLI setup
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

        CliTs::new(
            &self.schema.cli.name,
            self.schema.cli.version.clone(),
            self.schema.cli.description.clone(),
            commands,
        )
        .write(output_dir)?;

        // Ensure commands directory exists
        std::fs::create_dir_all(output_dir.join("src").join("commands"))?;

        // Generate individual command files (recursively for nested commands)
        for (name, command) in &self.schema.commands {
            self.generate_command_files_recursive(output_dir, name, command, vec![name.clone()])?;
        }

        // Generate handlers
        let result = self.generate_handlers(&handlers_dir, output_dir)?;

        Ok(result)
    }

    fn collect_context_fields(&self) -> Vec<ContextFieldInfo> {
        use baobao_manifest::ContextField;

        self.schema
            .context
            .fields()
            .into_iter()
            .map(|(name, field)| {
                let env_var = field
                    .env()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| field.default_env().to_string());

                let pool = field
                    .pool_config()
                    .map(|p| PoolConfigInfo {
                        max_connections: p.max_connections,
                        min_connections: p.min_connections,
                        acquire_timeout: p.acquire_timeout,
                        idle_timeout: p.idle_timeout,
                        max_lifetime: p.max_lifetime,
                    })
                    .unwrap_or_default();

                let sqlite = field.sqlite_config().map(|s| SqliteConfigInfo {
                    path: s.path.clone(),
                    create_if_missing: s.create_if_missing,
                    read_only: s.read_only,
                    journal_mode: s.journal_mode.as_ref().map(|m| m.as_str().to_string()),
                    synchronous: s.synchronous.as_ref().map(|m| m.as_str().to_string()),
                    busy_timeout: s.busy_timeout,
                    foreign_keys: s.foreign_keys,
                });

                let field_type = match &field {
                    ContextField::Postgres(_) => ContextFieldType::Database(DatabaseType::Postgres),
                    ContextField::Mysql(_) => ContextFieldType::Database(DatabaseType::Mysql),
                    ContextField::Sqlite(_) => ContextFieldType::Database(DatabaseType::Sqlite),
                    ContextField::Http(_) => ContextFieldType::Http,
                };

                ContextFieldInfo {
                    name: name.to_string(),
                    field_type,
                    env_var,
                    is_async: field.is_async(),
                    pool,
                    sqlite,
                }
            })
            .collect()
    }

    /// Recursively generate command files for a command and all its subcommands.
    fn generate_command_files_recursive(
        &self,
        output_dir: &Path,
        name: &str,
        command: &Command,
        path_segments: Vec<String>,
    ) -> Result<()> {
        // Generate this command's file
        let content = self.generate_command_file(name, command, &path_segments);

        // Ensure parent directory exists for nested commands
        if path_segments.len() > 1 {
            let mut dir_path = output_dir.join("src").join("commands");
            for segment in &path_segments[..path_segments.len() - 1] {
                dir_path = dir_path.join(to_kebab_case(segment));
            }
            std::fs::create_dir_all(&dir_path)?;
        }

        CommandTs::nested(path_segments.clone(), content).write(output_dir)?;

        // Recursively generate subcommand files
        if command.has_subcommands() {
            for (sub_name, sub_command) in &command.commands {
                let mut sub_path = path_segments.clone();
                sub_path.push(sub_name.clone());
                self.generate_command_files_recursive(output_dir, sub_name, sub_command, sub_path)?;
            }
        }

        Ok(())
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
        let mut created_handlers = Vec::new();

        // Collect all expected handler paths, using kebab-case for TypeScript file names
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

        // Ensure handlers directory exists
        std::fs::create_dir_all(handlers_dir)?;

        // Generate stub handlers for missing commands
        for (name, command) in &self.schema.commands {
            let created =
                self.generate_handler_stubs(handlers_dir, name, command, vec![name.clone()])?;
            created_handlers.extend(created);
        }

        // Find orphan handlers using shared utility
        let handler_paths = HandlerPaths::new(handlers_dir, "ts");
        let orphan_handlers = handler_paths.find_orphans(&expected_handlers)?;

        Ok(GenerateResult {
            created_handlers,
            orphan_handlers,
        })
    }

    /// Generate stub handler files for a command (recursively for subcommands).
    fn generate_handler_stubs(
        &self,
        handlers_dir: &Path,
        name: &str,
        command: &Command,
        path_segments: Vec<String>,
    ) -> Result<Vec<String>> {
        use baobao_core::WriteResult;

        let mut created = Vec::new();

        let kebab_name = to_kebab_case(name);
        let display_path = path_segments
            .iter()
            .map(|s| to_kebab_case(s))
            .collect::<Vec<_>>()
            .join("/");

        if command.has_subcommands() {
            // Create directory for subcommands
            let subdir = handlers_dir.join(&kebab_name);
            std::fs::create_dir_all(&subdir)?;

            // Recursively generate stubs for subcommands
            for (sub_name, sub_command) in &command.commands {
                let mut sub_path = path_segments.clone();
                sub_path.push(sub_name.clone());
                let sub_created =
                    self.generate_handler_stubs(&subdir, sub_name, sub_command, sub_path)?;
                created.extend(sub_created);
            }
        } else {
            // Leaf command - generate stub if file doesn't exist
            let has_args = !command.args.is_empty();
            let has_options = !command.flags.is_empty();
            let stub = HandlerTs::nested(name, path_segments, has_args, has_options);
            let result = stub.write(handlers_dir)?;

            if matches!(result, WriteResult::Written) {
                created.push(format!("{}.ts", display_path));
            }
        }

        Ok(created)
    }
}
