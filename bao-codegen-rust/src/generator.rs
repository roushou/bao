use std::{collections::HashSet, path::Path};

use baobao_codegen::{
    adapters::{CliAdapter, DatabaseAdapter, ErrorAdapter, RuntimeAdapter},
    builder::{
        AttributeSpec, CodeBuilder, EnumSpec, FieldSpec, StructSpec, StructureRenderer, TypeRef,
        VariantSpec, Visibility,
    },
    generation::{FileEntry, FileRegistry, HandlerPaths, find_orphan_commands},
    language::{CleanResult, GenerateResult, LanguageCodegen, PreviewFile},
    pipeline::CompilationContext,
    schema::ComputedData,
};
use baobao_core::{DatabaseType, GeneratedFile, to_pascal_case, to_snake_case};
use baobao_ir::{AppIR, CommandOp, InputKind, InputType, Operation, Resource};
use eyre::Result;

use crate::{
    Arm, ClapAdapter, ClapAttr, Enum, EyreAdapter, Field, Fn, Impl, Match, Param, RustFile,
    RustStructureRenderer, SqlxAdapter, Struct, TokioAdapter, Use, Variant,
    files::{
        AppRs, CargoToml, CliRs, CommandRs, CommandsMod, ContextRs, GeneratedMod, HandlerStub,
        HandlersMod, MainRs, STUB_MARKER,
    },
};

/// Rust code generator that produces clap-based CLI code
pub struct Generator {
    ir: AppIR,
    computed: ComputedData,
}

impl LanguageCodegen for Generator {
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
        }
    }

    /// Build a file registry with all generated files.
    ///
    /// This centralizes file registration, making generation declarative.
    /// Files are registered by category (Config, Infrastructure, Generated)
    /// and the registry handles ordering and write rules.
    fn build_registry(&self) -> FileRegistry {
        let mut registry = FileRegistry::new();

        // Use pre-computed data from pipeline
        let context_fields = self.computed.context_fields.clone();
        let is_async = self.computed.is_async;

        // Config files
        let dependencies = self.collect_dependencies(is_async);
        registry.register(FileEntry::config(
            "Cargo.toml",
            CargoToml::new(&self.ir.meta.name)
                .with_version_str(&self.ir.meta.version)
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

        // Collect commands from IR
        let commands: Vec<CommandOp> = self.ir.commands().cloned().collect();
        let command_names: Vec<String> = commands.iter().map(|c| c.name.clone()).collect();

        registry.register(FileEntry::generated(
            "src/generated/cli.rs",
            CliRs::new(
                &self.ir.meta.name,
                &self.ir.meta.version,
                self.ir.meta.description.clone(),
                commands,
                is_async,
            )
            .render(),
        ));

        registry.register(FileEntry::generated(
            "src/generated/commands/mod.rs",
            CommandsMod::new(command_names).render(),
        ));

        // Individual command files from IR
        for op in &self.ir.operations {
            let Operation::Command(cmd) = op;
            let content = self.generate_command_file_from_ir(cmd, is_async);
            let file_name = to_snake_case(&cmd.name);
            registry.register(FileEntry::generated(
                format!("src/generated/commands/{}.rs", file_name),
                CommandRs::new(&cmd.name, content).render(),
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
        let is_async = self.computed.is_async;

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

        // Collect expected command names from IR (snake_case for file names)
        let expected_commands: HashSet<String> = self
            .ir
            .operations
            .iter()
            .map(|op| {
                let Operation::Command(cmd) = op;
                to_snake_case(&cmd.name)
            })
            .collect();

        // Collect expected handler paths from computed data (convert to snake_case)
        let expected_handlers: HashSet<String> = self
            .computed
            .command_paths
            .iter()
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

        // Collect expected command names from IR (snake_case for file names)
        let expected_commands: HashSet<String> = self
            .ir
            .operations
            .iter()
            .map(|op| {
                let Operation::Command(cmd) = op;
                to_snake_case(&cmd.name)
            })
            .collect();

        // Collect expected handler paths from computed data (convert to snake_case)
        let expected_handlers: HashSet<String> = self
            .computed
            .command_paths
            .iter()
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

        // Add database and HTTP dependencies based on IR resources
        for resource in &self.ir.resources {
            match resource {
                Resource::Database(db) => {
                    let db_type = match db.db_type {
                        baobao_ir::DatabaseType::Postgres => DatabaseType::Postgres,
                        baobao_ir::DatabaseType::Mysql => DatabaseType::Mysql,
                        baobao_ir::DatabaseType::Sqlite => DatabaseType::Sqlite,
                    };
                    for dep in database.dependencies(db_type) {
                        if seen.insert(dep.name.clone()) {
                            dependencies.push((dep.name, dep.version));
                        }
                    }
                }
                Resource::HttpClient(_) => {
                    // Add reqwest for HTTP client
                    let reqwest = ("reqwest".to_string(), "0.12".to_string());
                    if seen.insert(reqwest.0.clone()) {
                        dependencies.push(reqwest);
                    }
                }
            }
        }

        dependencies
    }

    // ========================================================================
    // IR-based command generation methods
    // ========================================================================

    /// Generate a command file from IR CommandOp.
    fn generate_command_file_from_ir(&self, cmd: &CommandOp, is_async: bool) -> String {
        let pascal_name = to_pascal_case(&cmd.name);

        let mut file = RustFile::new().use_stmt(Use::new("clap").symbol("Args"));

        if cmd.has_subcommands() {
            file = file
                .use_stmt(Use::new("clap").symbol("Subcommand"))
                .use_stmt(Use::new("crate::context").symbol("Context"));
        }

        let content = if cmd.has_subcommands() {
            self.generate_subcommand_struct_from_ir(&cmd.name, &pascal_name, cmd, is_async)
        } else {
            self.generate_args_struct_from_ir(&pascal_name, cmd)
        };

        file.add(crate::RawCode::new(content))
            .render_with_header("// Generated by Bao - DO NOT EDIT")
    }

    /// Generate args struct from IR CommandOp using Code IR.
    fn generate_args_struct_from_ir(&self, pascal_name: &str, cmd: &CommandOp) -> String {
        let renderer = RustStructureRenderer::new();
        let mut builder = CodeBuilder::rust();

        // First, generate choice enums for inputs that have choices
        for input in &cmd.inputs {
            if let Some(choices) = &input.choices {
                let enum_name = format!("{}{}Choice", pascal_name, to_pascal_case(&input.name));
                let choice_enum = Self::generate_choice_enum(&enum_name, choices);
                builder.push_raw(&choice_enum);
                builder.push_blank();
            }
        }

        let mut spec = StructSpec::new(format!("{}Args", pascal_name))
            .doc(&cmd.description)
            .derive("Args")
            .derive("Debug");

        // Generate fields for all inputs
        for input in &cmd.inputs {
            let rust_type = if input.choices.is_some() {
                TypeRef::named(format!(
                    "{}{}Choice",
                    pascal_name,
                    to_pascal_case(&input.name)
                ))
            } else {
                Self::map_input_type_ref(input.ty)
            };

            let is_bool_flag = matches!(input.kind, InputKind::Flag { .. })
                && input.ty == InputType::Bool
                && input.choices.is_none();

            let field_type = if is_bool_flag {
                TypeRef::bool()
            } else if (input.required && input.default.is_none()) || input.default.is_some() {
                rust_type.clone()
            } else {
                TypeRef::optional(rust_type)
            };

            let mut field = FieldSpec::new(to_snake_case(&input.name), field_type)
                .visibility(Visibility::Public);

            if let Some(desc) = &input.description {
                field = field.doc(desc);
            }

            // Add clap attribute for flags
            if let InputKind::Flag { short } = &input.kind {
                let arg_attr = Self::build_clap_arg_attr(*short, input.default.as_ref());
                field = field.attribute(arg_attr);
            }

            spec = spec.field(field);
        }

        builder.push_raw(&renderer.render_struct(&spec));
        builder.build()
    }

    /// Build a clap arg attribute from flag parameters.
    fn build_clap_arg_attr(
        short: Option<char>,
        default: Option<&baobao_ir::DefaultValue>,
    ) -> AttributeSpec {
        let mut attr = AttributeSpec::simple("arg").flag("long");

        if let Some(c) = short {
            attr = attr.named("short", format!("'{}'", c));
        }

        if let Some(default_val) = default {
            attr = attr.named(
                "default_value",
                format!("\"{}\"", default_val.to_code_string()),
            );
        }

        attr
    }

    /// Map IR InputType to TypeRef.
    fn map_input_type_ref(input_type: InputType) -> TypeRef {
        match input_type {
            InputType::String => TypeRef::string(),
            InputType::Int => TypeRef::int(),
            InputType::Float => TypeRef::float(),
            InputType::Bool => TypeRef::bool(),
            InputType::Path => TypeRef::path(),
        }
    }

    /// Generate a clap ValueEnum for choices using Code IR.
    fn generate_choice_enum(name: &str, choices: &[String]) -> String {
        let renderer = RustStructureRenderer::new();

        let mut spec = EnumSpec::new(name)
            .derive("Debug")
            .derive("Clone")
            .derive("clap::ValueEnum");

        for choice in choices {
            let variant_name = to_pascal_case(choice);
            let attr = AttributeSpec::simple("clap").arg(format!("value(name = \"{}\")", choice));
            let variant = VariantSpec::unit(&variant_name).attribute(attr);
            spec = spec.variant(variant);
        }

        renderer.render_enum(&spec)
    }

    /// Generate handlers directory with mod.rs and stub files for missing handlers.
    fn generate_handlers(
        &self,
        handlers_dir: &Path,
        output_dir: &Path,
        is_async: bool,
    ) -> Result<GenerateResult> {
        let mut created_handlers = Vec::new();

        // Collect all expected handler paths from computed data (snake_case for Rust file names)
        let expected_handlers: HashSet<String> = self
            .computed
            .command_paths
            .iter()
            .map(|path| {
                path.split('/')
                    .map(to_snake_case)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .collect();

        // Collect top-level command names
        let top_level_names: Vec<String> = self
            .ir
            .operations
            .iter()
            .map(|op| {
                let Operation::Command(cmd) = op;
                cmd.name.clone()
            })
            .collect();

        // Generate top-level handlers/mod.rs (always regenerated)
        HandlersMod::new(top_level_names).write(output_dir)?;

        // Process commands recursively
        for op in &self.ir.operations {
            let Operation::Command(cmd) = op;
            self.generate_handlers_for_command(cmd, handlers_dir, is_async, &mut created_handlers)?;
        }

        // Find orphan handlers using shared utility
        let handler_paths = HandlerPaths::new(handlers_dir, "rs", STUB_MARKER);
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
        is_async: bool,
        created_handlers: &mut Vec<String>,
    ) -> Result<()> {
        use baobao_core::{File, WriteResult};

        let handler_path: Vec<&str> = cmd.path.iter().map(|s| s.as_str()).collect();
        let dir = handler_path
            .iter()
            .take(handler_path.len().saturating_sub(1))
            .fold(handlers_dir.to_path_buf(), |acc, segment| {
                acc.join(to_snake_case(segment))
            });

        if cmd.has_subcommands() {
            // Parent command - create directory and mod.rs
            let cmd_dir = dir.join(to_snake_case(&cmd.name));
            std::fs::create_dir_all(&cmd_dir)?;

            let subcommand_names: Vec<String> =
                cmd.children.iter().map(|c| c.name.clone()).collect();
            let handlers_mod = HandlersMod::new(subcommand_names);
            File::new(cmd_dir.join("mod.rs"), handlers_mod.render()).write()?;

            // Recursively process children
            for child in &cmd.children {
                self.generate_handlers_for_command(
                    child,
                    handlers_dir,
                    is_async,
                    created_handlers,
                )?;
            }
        } else {
            // Leaf command - create handler stub
            std::fs::create_dir_all(&dir)?;

            let display_path = cmd
                .path
                .iter()
                .map(|s| to_snake_case(s))
                .collect::<Vec<_>>()
                .join("/");
            let pascal_name = to_pascal_case(&cmd.name);

            // Args types are in the top-level command module
            let top_level_cmd = to_snake_case(cmd.path.first().unwrap_or(&cmd.name));
            let args_import = format!(
                "crate::generated::commands::{}::{}Args",
                top_level_cmd, pascal_name
            );

            let stub = HandlerStub::new(&cmd.name, &args_import, is_async);
            let result = stub.write(&dir)?;

            if matches!(result, WriteResult::Written) {
                created_handlers.push(format!("{}.rs", display_path));
            }
        }

        Ok(())
    }

    /// Generate subcommand struct from IR CommandOp.
    fn generate_subcommand_struct_from_ir(
        &self,
        handler_path: &str,
        pascal_name: &str,
        cmd: &CommandOp,
        is_async: bool,
    ) -> String {
        let await_suffix = if is_async { ".await" } else { "" };

        // Parent struct with subcommand field
        let parent_struct = Struct::new(pascal_name)
            .doc(&cmd.description)
            .derive("Args")
            .derive("Debug")
            .field(
                Field::new("command", format!("{}Commands", pascal_name))
                    .clap_attr(ClapAttr::command_subcommand()),
            );

        // Subcommands enum
        let mut commands_enum = Enum::new(format!("{}Commands", pascal_name))
            .derive("Subcommand")
            .derive("Debug");

        for child in &cmd.children {
            let sub_pascal = to_pascal_case(&child.name);
            let data = if child.has_subcommands() {
                sub_pascal.clone()
            } else {
                format!("{}Args", sub_pascal)
            };
            commands_enum = commands_enum.variant(
                Variant::new(&sub_pascal)
                    .doc(&child.description)
                    .tuple(data),
            );
        }

        // Dispatch impl
        let mut match_expr = Match::new("self.command");
        for child in &cmd.children {
            let sub_pascal = to_pascal_case(&child.name);
            let (pattern, body) = if child.has_subcommands() {
                (
                    format!("{}Commands::{}(cmd)", pascal_name, sub_pascal),
                    format!("cmd.dispatch(ctx){}", await_suffix),
                )
            } else {
                // Use snake_case for module paths
                let handler_module = handler_path
                    .split("::")
                    .map(to_snake_case)
                    .collect::<Vec<_>>()
                    .join("::");
                let sub_module = to_snake_case(&child.name);
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

        // Combine all parts
        let mut builder = CodeBuilder::rust();
        builder.emit(&parent_struct);
        builder.push_blank();
        builder.emit(&commands_enum);
        builder.push_blank();
        builder.emit(&dispatch_impl);
        builder.push_blank();

        // Generate args structs for each subcommand
        for child in &cmd.children {
            let sub_pascal = to_pascal_case(&child.name);
            if child.has_subcommands() {
                let nested_path = format!("{}::{}", handler_path, child.name);
                builder.push_raw(&self.generate_subcommand_struct_from_ir(
                    &nested_path,
                    &sub_pascal,
                    child,
                    is_async,
                ));
            } else {
                builder.push_raw(&self.generate_args_struct_from_ir(&sub_pascal, child));
            }
        }

        builder.build()
    }
}
