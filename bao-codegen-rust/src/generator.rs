use std::{collections::HashSet, path::Path};

use baobao_codegen::{
    adapters::{CliAdapter, DatabaseAdapter, ErrorAdapter, RuntimeAdapter},
    builder::CodeBuilder,
    generation::{FileEntry, FileRegistry, HandlerPaths, find_orphan_commands},
    language::{CleanResult, GenerateResult, LanguageCodegen, PreviewFile},
    schema::{CommandInfo, CommandTree, collect_context_fields},
};
use baobao_core::{
    DatabaseType, GeneratedFile, to_pascal_case, to_snake_case, toml_value_to_string,
};
use baobao_manifest::{ArgType, Command, Manifest};
use eyre::Result;

use crate::{
    Arm, ClapAdapter, Enum, EyreAdapter, Field, Fn, Impl, Match, Param, RustFile, SqlxAdapter,
    Struct, TokioAdapter, Use, Variant,
    files::{
        AppRs, CargoToml, CliRs, CommandRs, CommandsMod, ContextRs, GeneratedMod, HandlerStub,
        HandlersMod, MainRs,
    },
};

/// Marker string indicating an unmodified Rust handler stub.
const STUB_MARKER: &str = "todo!(\"implement";

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

    fn clean(&self, output_dir: &Path) -> Result<CleanResult> {
        self.clean_files(output_dir)
    }

    fn preview_clean(&self, output_dir: &Path) -> Result<CleanResult> {
        self.preview_clean_files(output_dir)
    }
}

impl<'a> Generator<'a> {
    pub fn new(schema: &'a Manifest) -> Self {
        Self { schema }
    }

    /// Build a file registry with all generated files.
    ///
    /// This centralizes file registration, making generation declarative.
    /// Files are registered by category (Config, Infrastructure, Generated)
    /// and the registry handles ordering and write rules.
    fn build_registry(&self) -> FileRegistry {
        let mut registry = FileRegistry::new();

        // Collect context field info
        let context_fields = collect_context_fields(&self.schema.context);
        // Async only if database context exists (HTTP is sync)
        let is_async = self.schema.context.has_async();

        // Config files
        let dependencies = self.collect_dependencies(is_async);
        registry.register(FileEntry::config(
            "Cargo.toml",
            CargoToml::new(&self.schema.cli.name)
                .with_version(self.schema.cli.version.clone())
                .with_dependencies(dependencies)
                .render(),
        ));

        // Infrastructure files
        registry.register(FileEntry::infrastructure(
            "src/main.rs",
            MainRs::new(is_async).render(),
        ));
        registry.register(FileEntry::infrastructure(
            "src/app.rs",
            AppRs::new(is_async).render(),
        ));
        registry.register(FileEntry::infrastructure(
            "src/context.rs",
            ContextRs::new(context_fields).render(),
        ));

        // Generated module files
        registry.register(FileEntry::generated(
            "src/generated/mod.rs",
            GeneratedMod.render(),
        ));

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
            "src/generated/cli.rs",
            CliRs::new(
                &self.schema.cli.name,
                self.schema.cli.version.clone(),
                self.schema.cli.description.clone(),
                commands,
                is_async,
            )
            .render(),
        ));

        registry.register(FileEntry::generated(
            "src/generated/commands/mod.rs",
            CommandsMod::new(self.schema.commands.keys().cloned().collect()).render(),
        ));

        // Individual command files
        for (name, command) in &self.schema.commands {
            let content = self.generate_command_file(name, command, is_async);
            let file_name = to_snake_case(name);
            registry.register(FileEntry::generated(
                format!("src/generated/commands/{}.rs", file_name),
                CommandRs::new(name, content).render(),
            ));
        }

        registry
    }

    /// Preview generated files without writing to disk
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

    /// Generate all files into the specified output directory
    fn generate_files(&self, output_dir: &Path) -> Result<GenerateResult> {
        let handlers_dir = output_dir.join("src/handlers");
        let is_async = self.schema.context.has_async();

        // Write all registered files using the registry
        let registry = self.build_registry();
        registry.write_all(output_dir)?;

        // Generate handlers (handled separately due to special logic)
        let result = self.generate_handlers(&handlers_dir, output_dir, is_async)?;

        Ok(result)
    }

    /// Clean orphaned generated files.
    fn clean_files(&self, output_dir: &Path) -> Result<CleanResult> {
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
        let commands_dir = output_dir.join("src/generated/commands");
        let orphan_commands = find_orphan_commands(&commands_dir, "rs", &expected_commands)?;
        for path in orphan_commands {
            std::fs::remove_file(&path)?;
            let relative = path.strip_prefix(output_dir).unwrap_or(&path);
            result.deleted_commands.push(relative.display().to_string());
        }

        // Find and handle orphaned handler files
        let handlers_dir = output_dir.join("src/handlers");
        let handler_paths = HandlerPaths::new(&handlers_dir, "rs", STUB_MARKER);
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

    /// Preview what would be cleaned without actually deleting files.
    fn preview_clean_files(&self, output_dir: &Path) -> Result<CleanResult> {
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
        let commands_dir = output_dir.join("src/generated/commands");
        let orphan_commands = find_orphan_commands(&commands_dir, "rs", &expected_commands)?;
        for path in orphan_commands {
            let relative = path.strip_prefix(output_dir).unwrap_or(&path);
            result.deleted_commands.push(relative.display().to_string());
        }

        // Find orphaned handler files
        let handlers_dir = output_dir.join("src/handlers");
        let handler_paths = HandlerPaths::new(&handlers_dir, "rs", STUB_MARKER);
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

    fn collect_dependencies(&self, has_async_context: bool) -> Vec<(String, String)> {
        // Use adapters to collect dependencies
        let cli = ClapAdapter::new();
        let error = EyreAdapter::new();
        let runtime = TokioAdapter::new();
        let database = SqlxAdapter::new();

        let mut dependencies: Vec<(String, String)> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();

        // Add error adapter dependencies
        for dep in error.dependencies() {
            if seen.insert(dep.name.clone()) {
                dependencies.push((dep.name, dep.version));
            }
        }

        // Add CLI adapter dependencies
        for dep in cli.dependencies() {
            if seen.insert(dep.name.clone()) {
                dependencies.push((dep.name, dep.version));
            }
        }

        // Add async runtime dependencies if needed
        if has_async_context {
            for dep in runtime.dependencies() {
                if seen.insert(dep.name.clone()) {
                    dependencies.push((dep.name, dep.version));
                }
            }
        }

        // Add database dependencies based on context fields
        use baobao_manifest::ContextField;

        for (_, field) in self.schema.context.fields() {
            // Get database dependencies from the adapter based on field type
            let db_type = match field {
                ContextField::Postgres(_) => Some(DatabaseType::Postgres),
                ContextField::Mysql(_) => Some(DatabaseType::Mysql),
                ContextField::Sqlite(_) => Some(DatabaseType::Sqlite),
                ContextField::Http(_) => None,
            };

            if let Some(db_type) = db_type {
                for dep in database.dependencies(db_type) {
                    if seen.insert(dep.name.clone()) {
                        dependencies.push((dep.name, dep.version));
                    }
                }
            }

            // Also include any additional dependencies from the field itself
            // (e.g., HTTP client reqwest)
            for (dep_name, dep_version) in field.dependencies() {
                if !seen.contains(dep_name) {
                    seen.insert(dep_name.to_string());
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
            let rust_type = Self::map_arg_type(&arg.arg_type);
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

            let rust_type = Self::map_arg_type(&flag.flag_type);
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

    /// Map manifest ArgType to Rust type string.
    fn map_arg_type(arg_type: &ArgType) -> &'static str {
        match arg_type {
            ArgType::String => "String",
            ArgType::Int => "i64",
            ArgType::Float => "f64",
            ArgType::Bool => "bool",
            ArgType::Path => "std::path::PathBuf",
        }
    }

    /// Generate handlers directory with mod.rs and stub files for missing handlers
    fn generate_handlers(
        &self,
        handlers_dir: &Path,
        output_dir: &Path,
        is_async: bool,
    ) -> Result<GenerateResult> {
        use baobao_core::{File, WriteResult};

        let mut created_handlers = Vec::new();
        let tree = CommandTree::new(self.schema);

        // Collect all expected handler paths (snake_case for Rust file names)
        let expected_handlers: HashSet<String> = tree
            .iter()
            .map(|cmd| cmd.path_transformed("/", to_snake_case))
            .collect();

        // Generate top-level handlers/mod.rs (always regenerated)
        HandlersMod::new(self.schema.commands.keys().cloned().collect()).write(output_dir)?;

        // Create mod.rs files for all parent commands (command groups)
        for cmd in tree.parents() {
            let dir = cmd.handler_dir(handlers_dir, to_snake_case);
            std::fs::create_dir_all(&dir)?;

            // Collect subcommand names for mod.rs
            let subcommand_names: Vec<String> = cmd.command.commands.keys().cloned().collect();
            let handlers_mod = HandlersMod::new(subcommand_names);
            File::new(dir.join("mod.rs"), handlers_mod.render()).write()?;
        }

        // Create stub files for all leaf commands (actual handlers)
        for cmd in tree.leaves() {
            let dir = cmd.handler_dir(handlers_dir, to_snake_case);
            std::fs::create_dir_all(&dir)?;

            let display_path = cmd.path_transformed("/", to_snake_case);
            let pascal_name = to_pascal_case(cmd.name);

            // Args types are in the top-level command module
            let top_level_cmd = to_snake_case(cmd.path.first().unwrap_or(&cmd.name));
            let args_import = format!(
                "crate::generated::commands::{}::{}Args",
                top_level_cmd, pascal_name
            );

            let stub = HandlerStub::new(cmd.name, &args_import, is_async);
            let result = stub.write(&dir)?;

            if matches!(result, WriteResult::Written) {
                created_handlers.push(format!("{}.rs", display_path));
            }
        }

        // Find orphan handlers using shared utility
        let handler_paths = HandlerPaths::new(handlers_dir, "rs", STUB_MARKER);
        let orphan_handlers = handler_paths.find_orphans(&expected_handlers)?;

        Ok(GenerateResult {
            created_handlers,
            orphan_handlers,
        })
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
