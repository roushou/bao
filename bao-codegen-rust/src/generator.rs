use std::{collections::HashSet, path::Path};

use baobao_codegen::{
    CleanResult, CodeBuilder, CommandInfo, CommandTree, ContextFieldInfo, GenerateResult,
    HandlerPaths, LanguageCodegen, PoolConfigInfo, PreviewFile, SqliteConfigInfo,
    find_orphan_commands,
};
use baobao_core::{
    ContextFieldType, DatabaseType, GeneratedFile, to_pascal_case, to_snake_case,
    toml_value_to_string,
};
use baobao_manifest::{ArgType, Command, Manifest};
use eyre::Result;

use crate::{
    Arm, Enum, Field, Fn, Impl, Match, Param, RustFile, Struct, Use, Variant,
    files::{
        AppRs, CargoToml, CliRs, CommandRs, CommandsMod, ContextRs, GeneratedMod, HandlerStub,
        HandlersMod, MainRs,
    },
};

/// Rust code generator that produces clap-based CLI code
pub struct Generator<'a> {
    schema: &'a Manifest,
}

impl LanguageCodegen for Generator<'_> {
    fn language(&self) -> &'static str {
        "rust"
    }

    fn file_extension(&self) -> &'static str {
        "rs"
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

    /// Preview generated files without writing to disk
    fn preview_files(&self) -> Vec<PreviewFile> {
        let mut files = Vec::new();

        // Collect context field info
        let context_fields = self.collect_context_fields();
        // Async only if database context exists (HTTP is sync)
        let is_async = self.schema.context.has_async();

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
                .with_version(self.schema.cli.version.clone())
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
                self.schema.cli.version.clone(),
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
            // Use snake_case for file names (handles dashed names like "my-command" -> "my_command")
            let file_name = to_snake_case(name);
            files.push(PreviewFile {
                path: format!("src/generated/commands/{}.rs", file_name),
                content: CommandRs::new(name, content).render(),
            });
        }

        files
    }

    /// Generate all files into the specified output directory
    fn generate_files(&self, output_dir: &Path) -> Result<GenerateResult> {
        let handlers_dir = output_dir.join("src").join("handlers");

        // Collect context field info
        let context_fields = self.collect_context_fields();
        // Async only if database context exists (HTTP is sync)
        let is_async = self.schema.context.has_async();

        // Generate context.rs
        ContextRs::new(context_fields).write(output_dir)?;

        // Generate main.rs
        MainRs::new(is_async).write(output_dir)?;

        // Generate app.rs
        AppRs::new(is_async).write(output_dir)?;

        // Generate Cargo.toml with all dependencies
        let dependencies = self.collect_dependencies(is_async);
        CargoToml::new(&self.schema.cli.name)
            .with_version(self.schema.cli.version.clone())
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
            self.schema.cli.version.clone(),
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

    /// Clean orphaned generated files
    ///
    /// Removes:
    /// - Generated command files in `src/generated/commands/` that are no longer in the manifest
    /// - Unmodified handler stubs in `src/handlers/` that are no longer in the manifest
    ///
    /// Handler files that have been modified by the user are not deleted.
    pub fn clean(&self, output_dir: &Path) -> Result<CleanResult> {
        let mut result = CleanResult::default();

        // Collect expected command names (snake_case for file names)
        let expected_commands: HashSet<String> = self
            .schema
            .commands
            .keys()
            .map(|name| to_snake_case(name))
            .collect();

        // Collect expected handler paths (snake_case)
        let expected_handlers: HashSet<String> = CommandTree::new(self.schema)
            .collect_paths()
            .into_iter()
            .map(|path| {
                path.split('/')
                    .map(to_snake_case)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .collect();

        // Find and delete orphaned generated command files
        let commands_dir = output_dir.join("src").join("generated").join("commands");
        let orphan_commands = find_orphan_commands(&commands_dir, "rs", &expected_commands)?;
        for path in orphan_commands {
            std::fs::remove_file(&path)?;
            let relative = path.strip_prefix(output_dir).unwrap_or(&path);
            result.deleted_commands.push(relative.display().to_string());
        }

        // Find and handle orphaned handler files
        let handlers_dir = output_dir.join("src").join("handlers");
        let handler_paths = HandlerPaths::new(&handlers_dir, "rs");
        let orphan_handlers = handler_paths.find_orphans_with_status(&expected_handlers)?;

        for orphan in orphan_handlers {
            if orphan.is_unmodified {
                // Safe to delete - it's still just a stub
                std::fs::remove_file(&orphan.full_path)?;
                result
                    .deleted_handlers
                    .push(format!("src/handlers/{}.rs", orphan.relative_path));

                // Try to clean up empty parent directories
                if let Some(parent) = orphan.full_path.parent() {
                    let _ = Self::remove_empty_dirs(parent, &handlers_dir);
                }
            } else {
                // User has modified this file, skip it
                result
                    .skipped_handlers
                    .push(format!("src/handlers/{}.rs", orphan.relative_path));
            }
        }

        Ok(result)
    }

    /// Preview what would be cleaned without actually deleting files
    pub fn preview_clean(&self, output_dir: &Path) -> Result<CleanResult> {
        let mut result = CleanResult::default();

        // Collect expected command names (snake_case for file names)
        let expected_commands: HashSet<String> = self
            .schema
            .commands
            .keys()
            .map(|name| to_snake_case(name))
            .collect();

        // Collect expected handler paths (snake_case)
        let expected_handlers: HashSet<String> = CommandTree::new(self.schema)
            .collect_paths()
            .into_iter()
            .map(|path| {
                path.split('/')
                    .map(to_snake_case)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .collect();

        // Find orphaned generated command files
        let commands_dir = output_dir.join("src").join("generated").join("commands");
        let orphan_commands = find_orphan_commands(&commands_dir, "rs", &expected_commands)?;
        for path in orphan_commands {
            let relative = path.strip_prefix(output_dir).unwrap_or(&path);
            result.deleted_commands.push(relative.display().to_string());
        }

        // Find orphaned handler files
        let handlers_dir = output_dir.join("src").join("handlers");
        let handler_paths = HandlerPaths::new(&handlers_dir, "rs");
        let orphan_handlers = handler_paths.find_orphans_with_status(&expected_handlers)?;

        for orphan in orphan_handlers {
            if orphan.is_unmodified {
                result
                    .deleted_handlers
                    .push(format!("src/handlers/{}.rs", orphan.relative_path));
            } else {
                result
                    .skipped_handlers
                    .push(format!("src/handlers/{}.rs", orphan.relative_path));
            }
        }

        Ok(result)
    }

    /// Remove empty directories up to but not including the base directory
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

                // Convert schema ContextField to core ContextFieldType
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

        for (_, field) in self.schema.context.fields() {
            for (dep_name, dep_version) in field.dependencies() {
                if seen_dependencies.insert(dep_name) {
                    dependencies.push((dep_name.to_string(), dep_version.to_string()));
                }
            }
        }

        dependencies
    }

    fn generate_command_file(&self, name: &str, command: &Command, is_async: bool) -> String {
        let pascal_name = to_pascal_case(name);

        let mut file = RustFile::new().use_stmt(Use::new("clap").symbol("Args"));

        if command.has_subcommands() {
            file = file
                .use_stmt(Use::new("clap").symbol("Subcommand"))
                .use_stmt(Use::new("crate::context").symbol("Context"));
        }

        let content = if command.has_subcommands() {
            self.generate_subcommand_struct(name, &pascal_name, command, is_async)
        } else {
            self.generate_args_struct(&pascal_name, command)
        };

        file.add(crate::RawCode::new(content))
            .render_with_header("// Generated by Bao - DO NOT EDIT")
    }

    fn generate_args_struct(&self, pascal_name: &str, command: &Command) -> String {
        let mut s = Struct::new(format!("{}Args", pascal_name))
            .doc(&command.description)
            .derive("Args")
            .derive("Debug");

        // Generate positional args
        for (arg_name, arg) in &command.args {
            let rust_type = arg.arg_type.rust_type();
            let field_type = if arg.required && arg.default.is_none() {
                rust_type.to_string()
            } else {
                format!("Option<{}>", rust_type)
            };

            let mut field = Field::new(to_snake_case(arg_name), field_type);
            if let Some(desc) = &arg.description {
                field = field.doc(desc);
            }
            s = s.field(field);
        }

        // Generate flags
        for (flag_name, flag) in &command.flags {
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

            let rust_type = flag.flag_type.rust_type();
            let field_type = if flag.flag_type == ArgType::Bool {
                "bool".to_string()
            } else if flag.default.is_some() {
                rust_type.to_string()
            } else {
                format!("Option<{}>", rust_type)
            };

            let mut field = Field::new(to_snake_case(flag_name), field_type)
                .attr(format!("arg({})", attrs.join(", ")));
            if let Some(desc) = &flag.description {
                field = field.doc(desc);
            }
            s = s.field(field);
        }

        s.build()
    }

    /// Generate handlers directory with mod.rs and stub files for missing handlers
    fn generate_handlers(
        &self,
        handlers_dir: &Path,
        output_dir: &Path,
        is_async: bool,
    ) -> Result<GenerateResult> {
        let mut created_handlers = Vec::new();

        // Collect all expected handler paths, converting to snake_case for Rust file names
        let expected_handlers: std::collections::HashSet<String> = CommandTree::new(self.schema)
            .collect_paths()
            .into_iter()
            .map(|path| {
                // Convert each segment of the path to snake_case
                path.split('/')
                    .map(to_snake_case)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .collect();

        // Generate handlers/mod.rs (always regenerated)
        HandlersMod::new(self.schema.commands.keys().cloned().collect()).write(output_dir)?;

        // Generate stub handlers for missing commands
        for (name, command) in &self.schema.commands {
            let created =
                self.generate_handler_stubs(handlers_dir, output_dir, name, command, "", is_async)?;
            created_handlers.extend(created);
        }

        // Find orphan handlers using shared utility
        let handler_paths = HandlerPaths::new(handlers_dir, "rs");
        let orphan_handlers = handler_paths.find_orphans(&expected_handlers)?;

        Ok(GenerateResult {
            created_handlers,
            orphan_handlers,
        })
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

        // Path for display/tracking purposes (use snake_case to match actual file names)
        let snake_name = to_snake_case(name);
        let display_path = if prefix.is_empty() {
            snake_name.clone()
        } else {
            format!("{}/{}", prefix, snake_name)
        };

        if command.has_subcommands() {
            // Create mod.rs for the subcommand directory
            // Use snake_case for directory names (handles dashed names)
            let dir_name = to_snake_case(name);
            let subdir = handlers_dir.join(&dir_name);
            let handlers_mod = HandlersMod::new(command.commands.keys().cloned().collect());
            File::new(subdir.join("mod.rs"), handlers_mod.render()).write()?;

            // Recursively generate stubs for subcommands
            for (sub_name, sub_command) in &command.commands {
                // Use snake_case for prefix to match directory structure
                let new_prefix = if prefix.is_empty() {
                    snake_name.clone()
                } else {
                    format!("{}/{}", prefix, snake_name)
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
            // Args types are always in the top-level command file, not nested modules
            // So we only use the first segment of the path for the import
            // Use snake_case for module paths (handles dashed names)
            let args_import = if prefix.is_empty() {
                format!(
                    "crate::generated::commands::{}::{}Args",
                    snake_name, pascal_name
                )
            } else {
                // Get only the first segment (top-level command name)
                // prefix is already snake_case from our recursive calls
                let top_level_cmd = prefix.split('/').next().unwrap_or(prefix);
                format!(
                    "crate::generated::commands::{}::{}Args",
                    top_level_cmd, pascal_name
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

    fn generate_subcommand_struct(
        &self,
        handler_path: &str,
        pascal_name: &str,
        command: &Command,
        is_async: bool,
    ) -> String {
        let await_suffix = if is_async { ".await" } else { "" };

        // Parent struct with subcommand field
        let parent_struct = Struct::new(pascal_name)
            .doc(&command.description)
            .derive("Args")
            .derive("Debug")
            .field(
                Field::new("command", format!("{}Commands", pascal_name))
                    .attr("command(subcommand)"),
            );

        // Subcommands enum
        let mut commands_enum = Enum::new(format!("{}Commands", pascal_name))
            .derive("Subcommand")
            .derive("Debug");

        for (sub_name, sub_command) in &command.commands {
            let sub_pascal = to_pascal_case(sub_name);
            let data = if sub_command.has_subcommands() {
                sub_pascal.clone()
            } else {
                format!("{}Args", sub_pascal)
            };
            commands_enum = commands_enum.variant(
                Variant::new(&sub_pascal)
                    .doc(&sub_command.description)
                    .tuple(data),
            );
        }

        // Dispatch impl
        let mut match_expr = Match::new("self.command");
        for (sub_name, sub_command) in &command.commands {
            let sub_pascal = to_pascal_case(sub_name);
            let (pattern, body) = if sub_command.has_subcommands() {
                (
                    format!("{}Commands::{}(cmd)", pascal_name, sub_pascal),
                    format!("cmd.dispatch(ctx){}", await_suffix),
                )
            } else {
                // Use snake_case for module paths (handles dashed names like "my-command" -> "my_command")
                // handler_path uses :: as separator (e.g., "db::migrate"), convert each segment
                let handler_module = handler_path
                    .split("::")
                    .map(to_snake_case)
                    .collect::<Vec<_>>()
                    .join("::");
                let sub_module = to_snake_case(sub_name);
                (
                    format!("{}Commands::{}(args)", pascal_name, sub_pascal),
                    format!(
                        "crate::handlers::{}::{}::run(ctx, args){}",
                        handler_module, sub_module, await_suffix
                    ),
                )
            };
            match_expr = match_expr.arm(Arm::new(pattern).body(body));
        }

        let mut dispatch = Fn::new("dispatch")
            .doc("Dispatch the parsed subcommand to the appropriate handler")
            .param(Param::new("self", ""))
            .param(Param::new("ctx", "&Context"))
            .returns("eyre::Result<()>")
            .body_match(&match_expr);

        if is_async {
            dispatch = dispatch.async_();
        }

        let dispatch_impl = Impl::new(pascal_name).method(dispatch);

        // Combine all parts using mutable CodeBuilder
        let mut builder = CodeBuilder::rust();
        builder.emit(&parent_struct);
        builder.push_blank();
        builder.emit(&commands_enum);
        builder.push_blank();
        builder.emit(&dispatch_impl);
        builder.push_blank();

        // Generate args structs for each subcommand
        for (sub_name, sub_command) in &command.commands {
            let sub_pascal = to_pascal_case(sub_name);
            if sub_command.has_subcommands() {
                let nested_path = format!("{}::{}", handler_path, sub_name);
                builder.push_raw(&self.generate_subcommand_struct(
                    &nested_path,
                    &sub_pascal,
                    sub_command,
                    is_async,
                ));
            } else {
                builder.push_raw(&self.generate_args_struct(&sub_pascal, sub_command));
            }
        }

        builder.build()
    }
}
