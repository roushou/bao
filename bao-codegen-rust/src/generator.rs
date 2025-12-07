use std::{collections::HashSet, path::Path};

use baobao_core::{CommandInfo, ContextFieldInfo, GeneratedFile, PoolConfigInfo, SqliteConfigInfo};
use baobao_schema::{ArgType, Command, Schema};
use eyre::Result;

use crate::files::{
    AppRs, CargoToml, CliRs, CommandRs, CommandsMod, ContextRs, GeneratedMod, HandlerStub,
    HandlersMod, MainRs,
};

/// Result of code generation
pub struct GenerateResult {
    /// Handler files that were created (stubs for new commands)
    pub created_handlers: Vec<String>,
    /// Handler files that exist but are no longer used
    pub orphan_handlers: Vec<String>,
}

/// A generated file for preview
pub struct PreviewFile {
    /// Relative path from output directory
    pub path: String,
    /// File content
    pub content: String,
}

/// Rust code generator that produces clap-based CLI code
pub struct Generator<'a> {
    schema: &'a Schema,
}

impl<'a> Generator<'a> {
    pub fn new(schema: &'a Schema) -> Self {
        Self { schema }
    }

    /// Preview generated files without writing to disk
    pub fn preview(&self) -> Vec<PreviewFile> {
        let mut files = Vec::new();

        // Collect context field info
        let context_fields = self.collect_context_fields();
        // Async if any context field exists
        let is_async = !self.schema.context.is_empty();

        // context.rs
        files.push(PreviewFile {
            path: "src/context.rs".to_string(),
            content: ContextRs::new(context_fields).render(),
        });

        // main.rs
        files.push(PreviewFile {
            path: "src/main.rs".to_string(),
            content: MainRs::new(is_async).render(),
        });

        // app.rs
        files.push(PreviewFile {
            path: "src/app.rs".to_string(),
            content: AppRs::new(is_async).render(),
        });

        // Cargo.toml
        let dependencies = self.collect_dependencies(is_async);
        files.push(PreviewFile {
            path: "Cargo.toml".to_string(),
            content: CargoToml::new(&self.schema.cli.name)
                .with_version(&self.schema.cli.version)
                .with_dependencies(dependencies)
                .render(),
        });

        // generated/mod.rs
        files.push(PreviewFile {
            path: "src/generated/mod.rs".to_string(),
            content: GeneratedMod.render(),
        });

        // cli.rs
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
            path: "src/generated/cli.rs".to_string(),
            content: CliRs::new(
                &self.schema.cli.name,
                &self.schema.cli.version,
                self.schema.cli.description.clone(),
                commands,
                is_async,
            )
            .render(),
        });

        // commands/mod.rs
        files.push(PreviewFile {
            path: "src/generated/commands/mod.rs".to_string(),
            content: CommandsMod::new(self.schema.commands.keys().cloned().collect()).render(),
        });

        // Individual command files
        for (name, command) in &self.schema.commands {
            let content = self.generate_command_file(name, command, is_async);
            files.push(PreviewFile {
                path: format!("src/generated/commands/{}.rs", name),
                content: CommandRs::new(name, content).render(),
            });
        }

        files
    }

    /// Generate all files into the specified output directory
    pub fn generate(&self, output_dir: &Path) -> Result<GenerateResult> {
        let handlers_dir = output_dir.join("src").join("handlers");

        // Collect context field info
        let context_fields = self.collect_context_fields();
        // Async if any context field exists
        let is_async = !self.schema.context.is_empty();

        // Generate context.rs
        ContextRs::new(context_fields).write(output_dir)?;

        // Generate main.rs
        MainRs::new(is_async).write(output_dir)?;

        // Generate app.rs
        AppRs::new(is_async).write(output_dir)?;

        // Generate Cargo.toml with all dependencies
        let dependencies = self.collect_dependencies(is_async);
        CargoToml::new(&self.schema.cli.name)
            .with_version(&self.schema.cli.version)
            .with_dependencies(dependencies)
            .write(output_dir)?;

        // Generate mod.rs for generated module
        GeneratedMod.write(output_dir)?;

        // Generate cli.rs with main Cli struct and dispatch
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

        CliRs::new(
            &self.schema.cli.name,
            &self.schema.cli.version,
            self.schema.cli.description.clone(),
            commands,
            is_async,
        )
        .write(output_dir)?;

        // Generate commands/mod.rs
        CommandsMod::new(self.schema.commands.keys().cloned().collect()).write(output_dir)?;

        // Generate individual command files
        for (name, command) in &self.schema.commands {
            let content = self.generate_command_file(name, command, is_async);
            CommandRs::new(name, content).write(output_dir)?;
        }

        // Generate handlers
        let result = self.generate_handlers(&handlers_dir, output_dir, is_async)?;

        Ok(result)
    }

    fn collect_context_fields(&self) -> Vec<ContextFieldInfo> {
        self.schema
            .context
            .iter()
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
                    create_if_missing: s.create_if_missing,
                    read_only: s.read_only,
                    journal_mode: s.journal_mode.as_ref().map(|m| m.as_str().to_string()),
                    synchronous: s.synchronous.as_ref().map(|m| m.as_str().to_string()),
                    busy_timeout: s.busy_timeout,
                    foreign_keys: s.foreign_keys,
                });

                ContextFieldInfo {
                    name: name.clone(),
                    rust_type: field.rust_type().to_string(),
                    env_var,
                    is_async: field.is_async(),
                    pool,
                    sqlite,
                }
            })
            .collect()
    }

    fn collect_dependencies(&self, has_async_context: bool) -> Vec<(String, String)> {
        let mut dependencies: Vec<(String, String)> = vec![
            ("eyre".to_string(), "0.6".to_string()),
            (
                "clap".to_string(),
                r#"{ version = "4", features = ["derive"] }"#.to_string(),
            ),
        ];

        let mut seen_dependencies: HashSet<&str> = HashSet::from(["eyre", "clap"]);

        if has_async_context {
            dependencies.push((
                "tokio".to_string(),
                r#"{ version = "1", features = ["rt-multi-thread", "macros"] }"#.to_string(),
            ));
            seen_dependencies.insert("tokio");
        }

        for field in self.schema.context.values() {
            for (dep_name, dep_version) in field.dependencies() {
                if seen_dependencies.insert(dep_name) {
                    dependencies.push((dep_name.to_string(), dep_version.to_string()));
                }
            }
        }

        dependencies
    }

    fn generate_command_file(&self, name: &str, command: &Command, is_async: bool) -> String {
        let mut out = String::new();
        let pascal_name = to_pascal_case(name);

        out.push_str("// Generated by Bao - DO NOT EDIT\n\n");
        if command.has_subcommands() {
            out.push_str("use clap::{Args, Subcommand};\n\n");
            out.push_str("use crate::context::Context;\n\n");
        } else {
            out.push_str("use clap::Args;\n\n");
        }

        if command.has_subcommands() {
            // Command with subcommands
            out.push_str(&self.generate_subcommand_struct(name, &pascal_name, command, is_async));
        } else {
            // Leaf command with args
            out.push_str(&self.generate_args_struct(&pascal_name, command));
        }

        out
    }

    fn generate_args_struct(&self, pascal_name: &str, command: &Command) -> String {
        let mut out = String::new();

        out.push_str(&format!("/// {}\n", command.description));
        out.push_str("#[derive(Args, Debug)]\n");
        out.push_str(&format!("pub struct {}Args {{\n", pascal_name));

        // Generate positional args
        for (arg_name, arg) in &command.args {
            if let Some(desc) = &arg.description {
                out.push_str(&format!("    /// {}\n", desc));
            }

            let rust_type = arg.arg_type.rust_type();
            let field_type = if arg.required && arg.default.is_none() {
                rust_type.to_string()
            } else {
                format!("Option<{}>", rust_type)
            };

            out.push_str(&format!(
                "    pub {}: {},\n",
                to_snake_case(arg_name),
                field_type
            ));
        }

        // Generate flags
        for (flag_name, flag) in &command.flags {
            if let Some(desc) = &flag.description {
                out.push_str(&format!("    /// {}\n", desc));
            }

            let mut attrs = vec!["long".to_string()];
            if let Some(short) = flag.short_char() {
                attrs.push(format!("short = '{}'", short));
            }
            if let Some(default) = &flag.default {
                attrs.push(format!(
                    "default_value = \"{}\"",
                    toml_value_to_string(default)
                ));
            }

            out.push_str(&format!("    #[arg({})]\n", attrs.join(", ")));

            let rust_type = flag.flag_type.rust_type();
            let field_type = if flag.flag_type == ArgType::Bool {
                "bool".to_string()
            } else if flag.default.is_some() {
                rust_type.to_string()
            } else {
                format!("Option<{}>", rust_type)
            };

            out.push_str(&format!(
                "    pub {}: {},\n",
                to_snake_case(flag_name),
                field_type
            ));
        }

        out.push_str("}\n");
        out
    }

    /// Generate handlers directory with mod.rs and stub files for missing handlers
    fn generate_handlers(
        &self,
        handlers_dir: &Path,
        output_dir: &Path,
        is_async: bool,
    ) -> Result<GenerateResult> {
        let mut created_handlers = Vec::new();
        let mut expected_handlers = HashSet::new();

        // Collect all leaf commands (commands without subcommands)
        self.collect_leaf_commands(&self.schema.commands, "", &mut expected_handlers);

        // Generate handlers/mod.rs (always regenerated)
        HandlersMod::new(self.schema.commands.keys().cloned().collect()).write(output_dir)?;

        // Generate stub handlers for missing commands
        for (name, command) in &self.schema.commands {
            let created =
                self.generate_handler_stubs(handlers_dir, output_dir, name, command, "", is_async)?;
            created_handlers.extend(created);
        }

        // Find orphan handlers
        let orphan_handlers = self.find_orphan_handlers(handlers_dir, &expected_handlers)?;

        Ok(GenerateResult {
            created_handlers,
            orphan_handlers,
        })
    }

    /// Collect all leaf command paths (e.g., "hello", "db/migrate")
    fn collect_leaf_commands(
        &self,
        commands: &std::collections::HashMap<String, Command>,
        prefix: &str,
        result: &mut HashSet<String>,
    ) {
        for (name, command) in commands {
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", prefix, name)
            };

            if command.has_subcommands() {
                result.insert(path.clone()); // Parent directory
                self.collect_leaf_commands(&command.commands, &path, result);
            } else {
                result.insert(path);
            }
        }
    }

    /// Generate stub handler files for a command (recursively for subcommands)
    fn generate_handler_stubs(
        &self,
        handlers_dir: &Path,
        _output_dir: &Path,
        name: &str,
        command: &Command,
        prefix: &str,
        is_async: bool,
    ) -> Result<Vec<String>> {
        use baobao_core::{File, WriteResult};

        let mut created = Vec::new();

        // Path for display/tracking purposes
        let display_path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{}/{}", prefix, name)
        };

        if command.has_subcommands() {
            // Create mod.rs for the subcommand directory
            let subdir = handlers_dir.join(name);
            let handlers_mod = HandlersMod::new(command.commands.keys().cloned().collect());
            File::new(subdir.join("mod.rs"), handlers_mod.render()).write()?;

            // Recursively generate stubs for subcommands
            for (sub_name, sub_command) in &command.commands {
                let new_prefix = if prefix.is_empty() {
                    name.to_string()
                } else {
                    format!("{}/{}", prefix, name)
                };
                let sub_created = self.generate_handler_stubs(
                    &subdir,
                    _output_dir,
                    sub_name,
                    sub_command,
                    &new_prefix,
                    is_async,
                )?;
                created.extend(sub_created);
            }
        } else {
            // Leaf command - generate stub if file doesn't exist
            let pascal_name = to_pascal_case(name);
            let args_import = if prefix.is_empty() {
                format!("crate::generated::commands::{}Args", pascal_name)
            } else {
                format!(
                    "crate::generated::commands::{}::{}Args",
                    prefix, pascal_name
                )
            };

            let stub = HandlerStub::new(name, &args_import, is_async);
            let result = stub.write(handlers_dir)?;

            if matches!(result, WriteResult::Written) {
                created.push(format!("{}.rs", display_path));
            }
        }

        Ok(created)
    }

    /// Find handler files that exist but are no longer used
    fn find_orphan_handlers(
        &self,
        handlers_dir: &Path,
        expected: &HashSet<String>,
    ) -> Result<Vec<String>> {
        let mut orphans = Vec::new();
        self.scan_handler_files(handlers_dir, "", expected, &mut orphans)?;
        Ok(orphans)
    }

    /// Recursively scan for .rs files and find orphans
    fn scan_handler_files(
        &self,
        dir: &Path,
        prefix: &str,
        expected: &HashSet<String>,
        orphans: &mut Vec<String>,
    ) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_name = path.file_name().unwrap().to_string_lossy();

            if file_name == "mod.rs" {
                continue; // Skip mod.rs files
            }

            if path.is_dir() {
                let new_prefix = if prefix.is_empty() {
                    file_name.to_string()
                } else {
                    format!("{}/{}", prefix, file_name)
                };

                // Check if this directory is expected
                if !expected.contains(&new_prefix) {
                    orphans.push(new_prefix.clone());
                } else {
                    self.scan_handler_files(&path, &new_prefix, expected, orphans)?;
                }
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let name = path.file_stem().unwrap().to_string_lossy();
                let handler_path = if prefix.is_empty() {
                    name.to_string()
                } else {
                    format!("{}/{}", prefix, name)
                };

                if !expected.contains(&handler_path) {
                    orphans.push(handler_path);
                }
            }
        }

        Ok(())
    }

    fn generate_subcommand_struct(
        &self,
        name: &str,
        pascal_name: &str,
        command: &Command,
        is_async: bool,
    ) -> String {
        let mut out = String::new();

        // Parent struct with subcommand field
        out.push_str(&format!("/// {}\n", command.description));
        out.push_str("#[derive(Args, Debug)]\n");
        out.push_str(&format!("pub struct {} {{\n", pascal_name));
        out.push_str("    #[command(subcommand)]\n");
        out.push_str(&format!("    pub command: {}Commands,\n", pascal_name));
        out.push_str("}\n\n");

        // Subcommands enum
        out.push_str("#[derive(Subcommand, Debug)]\n");
        out.push_str(&format!("pub enum {}Commands {{\n", pascal_name));
        for (sub_name, sub_command) in &command.commands {
            let sub_pascal = to_pascal_case(sub_name);
            out.push_str(&format!("    /// {}\n", sub_command.description));
            if sub_command.has_subcommands() {
                out.push_str(&format!("    {}({}),\n", sub_pascal, sub_pascal));
            } else {
                out.push_str(&format!("    {}({}Args),\n", sub_pascal, sub_pascal));
            }
        }
        out.push_str("}\n\n");

        // Impl block with dispatch method
        let await_suffix = if is_async { ".await" } else { "" };
        out.push_str(&format!("impl {} {{\n", pascal_name));
        out.push_str("    /// Dispatch the parsed subcommand to the appropriate handler\n");
        if is_async {
            out.push_str("    pub async fn dispatch(self, ctx: &Context) -> eyre::Result<()> {\n");
        } else {
            out.push_str("    pub fn dispatch(self, ctx: &Context) -> eyre::Result<()> {\n");
        }
        out.push_str("        match self.command {\n");
        for (sub_name, sub_command) in &command.commands {
            let sub_pascal = to_pascal_case(sub_name);
            if sub_command.has_subcommands() {
                out.push_str(&format!(
                    "            {}Commands::{}(cmd) => cmd.dispatch(ctx){},\n",
                    pascal_name, sub_pascal, await_suffix
                ));
            } else {
                out.push_str(&format!(
                    "            {}Commands::{}(args) => crate::handlers::{}::{}::run(ctx, args){},\n",
                    pascal_name, sub_pascal, name, sub_name, await_suffix
                ));
            }
        }
        out.push_str("        }\n");
        out.push_str("    }\n");
        out.push_str("}\n\n");

        // Generate args structs for each subcommand
        for (sub_name, sub_command) in &command.commands {
            let sub_pascal = to_pascal_case(sub_name);
            if sub_command.has_subcommands() {
                out.push_str(&self.generate_subcommand_struct(
                    sub_name,
                    &sub_pascal,
                    sub_command,
                    is_async,
                ));
            } else {
                out.push_str(&self.generate_args_struct(&sub_pascal, sub_command));
            }
        }

        out
    }
}

fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}

fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap());
    }
    result.replace('-', "_")
}

fn toml_value_to_string(value: &toml::Value) -> String {
    match value {
        toml::Value::String(s) => s.clone(),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        _ => String::new(),
    }
}
