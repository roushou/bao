//! TypeScript code generator using boune framework.

use std::{collections::HashSet, path::Path};

use baobao_codegen::{
    CommandInfo, CommandTree, ContextFieldInfo, GenerateResult, HandlerPaths, LanguageCodegen,
    PoolConfigInfo, PreviewFile, SqliteConfigInfo,
};
use baobao_core::{
    ContextFieldType, DatabaseType, GeneratedFile, to_camel_case, to_kebab_case, to_pascal_case,
    toml_value_to_string,
};
use baobao_manifest::{ArgType, Command, Language, Manifest};
use eyre::Result;

use crate::files::{
    BaoToml, CliTs, CommandTs, ContextTs, GitIgnore, HandlerTs, IndexTs, PackageJson, TsConfig,
};

/// TypeScript code generator that produces boune-based CLI code for Bun.
pub struct Generator<'a> {
    schema: &'a Manifest,
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
        Self { schema }
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
        let camel_name = to_camel_case(name);
        let kebab_name = to_kebab_case(name);

        let mut code = String::new();

        // Imports
        code.push_str("import { command } from \"boune\";\n");

        // Import subcommands from the subdirectory
        for subcommand_name in command.commands.keys() {
            let sub_camel = to_camel_case(subcommand_name);
            let sub_kebab = to_kebab_case(subcommand_name);
            code.push_str(&format!(
                "import {{ {}Command }} from \"./{}/{}.ts\";\n",
                sub_camel, kebab_name, sub_kebab
            ));
        }
        code.push('\n');

        // Command definition with subcommands
        code.push_str(&format!(
            "export const {}Command = command(\"{}\")\n",
            camel_name, name
        ));
        code.push_str(&format!("  .description(\"{}\")\n", command.description));

        // Add subcommands
        for subcommand_name in command.commands.keys() {
            let sub_camel = to_camel_case(subcommand_name);
            code.push_str(&format!("  .subcommand({}Command)\n", sub_camel));
        }

        // Remove trailing newline from last .subcommand() line
        if code.ends_with('\n') {
            code.pop();
        }
        code.push_str(";\n");

        code
    }

    /// Generate a leaf command file that has an action handler.
    fn generate_leaf_command_file(
        &self,
        name: &str,
        command: &Command,
        path_segments: &[String],
    ) -> String {
        let pascal_name = to_pascal_case(name);
        let camel_name = to_camel_case(name);

        // Build the handler path (kebab-case, joined by /)
        let handler_path = path_segments
            .iter()
            .map(|s| to_kebab_case(s))
            .collect::<Vec<_>>()
            .join("/");

        // Calculate relative path to handlers from this command's location
        // For a command at commands/data/builders/leaderboard.ts,
        // handlers are at handlers/data/builders/leaderboard.ts
        // So we need "../../../handlers/data/builders/leaderboard.ts"
        let depth = path_segments.len();
        let up_path = "../".repeat(depth);

        let mut code = String::new();

        // Imports
        code.push_str("import { command } from \"boune\";\n");
        code.push_str(&format!(
            "import {{ run }} from \"{}handlers/{}.ts\";\n",
            up_path, handler_path
        ));
        code.push_str(&format!(
            "import type {{ Context }} from \"{}context.ts\";\n\n",
            up_path
        ));

        // Args interface
        code.push_str(&self.generate_args_interface(&pascal_name, command));
        code.push('\n');

        // Command definition
        code.push_str(&self.generate_command_definition(&pascal_name, &camel_name, name, command));

        code
    }

    fn generate_args_interface(&self, pascal_name: &str, command: &Command) -> String {
        let mut code = format!("export interface {}Args {{\n", pascal_name);

        // Positional args
        for (arg_name, arg) in &command.args {
            let ts_type = self.map_arg_type(&arg.arg_type);
            let camel_name = to_camel_case(arg_name);
            if arg.required && arg.default.is_none() {
                code.push_str(&format!("  {}: {};\n", camel_name, ts_type));
            } else {
                code.push_str(&format!("  {}?: {};\n", camel_name, ts_type));
            }
        }

        // Flags
        for (flag_name, flag) in &command.flags {
            let ts_type = self.map_arg_type(&flag.flag_type);
            let camel_name = to_camel_case(flag_name);
            // Bool flags always have a value (default false), and flags with defaults are required
            if flag.flag_type == ArgType::Bool || flag.default.is_some() {
                code.push_str(&format!("  {}: {};\n", camel_name, ts_type));
            } else {
                code.push_str(&format!("  {}?: {};\n", camel_name, ts_type));
            }
        }

        code.push_str("}\n");
        code
    }

    fn generate_command_definition(
        &self,
        pascal_name: &str,
        camel_name: &str,
        name: &str,
        command: &Command,
    ) -> String {
        let mut code = format!(
            "export const {}Command = command(\"{}\")\n",
            camel_name, name
        );
        code.push_str(&format!("  .description(\"{}\")\n", command.description));

        // Positional args
        for (arg_name, arg) in &command.args {
            let bracket = if arg.required && arg.default.is_none() {
                format!("<{}>", arg_name)
            } else {
                format!("[{}]", arg_name)
            };
            let desc = arg.description.as_deref().unwrap_or("");

            if arg.arg_type != ArgType::String {
                code.push_str(&format!(
                    "  .argument(\"{}\", \"{}\", {{ type: \"{}\" }})\n",
                    bracket,
                    desc,
                    self.map_boune_type(&arg.arg_type)
                ));
            } else {
                code.push_str(&format!("  .argument(\"{}\", \"{}\")\n", bracket, desc));
            }
        }

        // Flags
        for (flag_name, flag) in &command.flags {
            let short_part = flag
                .short_char()
                .map(|c| format!("-{}, ", c))
                .unwrap_or_default();
            let desc = flag.description.as_deref().unwrap_or("");

            if flag.flag_type == ArgType::Bool {
                code.push_str(&format!(
                    "  .option(\"{}--{}\", \"{}\")\n",
                    short_part, flag_name, desc
                ));
            } else {
                let default_part = flag.default.as_ref().map_or(String::new(), |d| {
                    format!(", default: {}", toml_value_to_string(d))
                });
                code.push_str(&format!(
                    "  .option(\"{}--{} <{}>\", \"{}\", {{ type: \"{}\"{} }})\n",
                    short_part,
                    flag_name,
                    flag_name,
                    desc,
                    self.map_boune_type(&flag.flag_type),
                    default_part
                ));
            }
        }

        // Action handler
        code.push_str("  .action(async ({ args, options }) => {\n");
        code.push_str(&format!("    const typedArgs: {}Args = {{\n", pascal_name));

        // Map args
        for arg_name in command.args.keys() {
            let camel = to_camel_case(arg_name);
            code.push_str(&format!(
                "      {}: args.{} as {}Args[\"{}\"],\n",
                camel, arg_name, pascal_name, camel
            ));
        }

        // Map flags
        for (flag_name, flag) in &command.flags {
            let camel = to_camel_case(flag_name);
            if flag.flag_type == ArgType::Bool {
                code.push_str(&format!(
                    "      {}: (options.{} as boolean) ?? false,\n",
                    camel, flag_name
                ));
            } else {
                code.push_str(&format!(
                    "      {}: options.{} as {}Args[\"{}\"],\n",
                    camel, flag_name, pascal_name, camel
                ));
            }
        }

        code.push_str("    };\n");
        code.push_str("    await run({} as Context, typedArgs);\n");
        code.push_str("  });\n");

        code
    }

    fn map_arg_type(&self, arg_type: &ArgType) -> &'static str {
        match arg_type {
            ArgType::String => "string",
            ArgType::Int => "number",
            ArgType::Float => "number",
            ArgType::Bool => "boolean",
            ArgType::Path => "string",
        }
    }

    fn map_boune_type(&self, arg_type: &ArgType) -> &'static str {
        match arg_type {
            ArgType::String => "string",
            ArgType::Int => "number",
            ArgType::Float => "number",
            ArgType::Bool => "boolean",
            ArgType::Path => "string",
        }
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
            let created = self.generate_handler_stubs(handlers_dir, name, command, "")?;
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
        prefix: &str,
    ) -> Result<Vec<String>> {
        use baobao_core::WriteResult;

        let mut created = Vec::new();

        let kebab_name = to_kebab_case(name);
        let display_path = if prefix.is_empty() {
            kebab_name.clone()
        } else {
            format!("{}/{}", prefix, kebab_name)
        };

        if command.has_subcommands() {
            // Create directory for subcommands
            let subdir = handlers_dir.join(&kebab_name);
            std::fs::create_dir_all(&subdir)?;

            // Recursively generate stubs for subcommands
            for (sub_name, sub_command) in &command.commands {
                let new_prefix = if prefix.is_empty() {
                    kebab_name.clone()
                } else {
                    format!("{}/{}", prefix, kebab_name)
                };
                let sub_created =
                    self.generate_handler_stubs(&subdir, sub_name, sub_command, &new_prefix)?;
                created.extend(sub_created);
            }
        } else {
            // Leaf command - generate stub if file doesn't exist
            let pascal_name = to_pascal_case(name);
            let stub = HandlerTs::new(name, format!("{}Args", pascal_name));
            let result = stub.write(handlers_dir)?;

            if matches!(result, WriteResult::Written) {
                created.push(format!("{}.ts", display_path));
            }
        }

        Ok(created)
    }
}
