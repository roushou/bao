//! Clap CLI framework adapter.

use baobao_codegen::{
    adapters::{CliAdapter, CliInfo, CommandMeta, Dependency, DispatchInfo, ImportSpec},
    builder::CodeFragment,
};
use baobao_core::ArgType;

use crate::{Arm, Enum, Field, Fn, Impl, Match, Param, Struct, Variant};

/// Clap adapter for generating derive-based CLI code.
#[derive(Debug, Clone, Default)]
pub struct ClapAdapter;

impl ClapAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl CliAdapter for ClapAdapter {
    fn name(&self) -> &'static str {
        "clap"
    }

    fn dependencies(&self) -> Vec<Dependency> {
        vec![Dependency::new(
            "clap",
            r#"{ version = "4", features = ["derive"] }"#,
        )]
    }

    fn generate_cli(&self, info: &CliInfo) -> Vec<CodeFragment> {
        let mut fragments = Vec::new();

        // Build CLI struct
        let mut s = Struct::new("Cli")
            .derive("Parser")
            .derive("Debug")
            .attr(format!("command(name = \"{}\")", info.name))
            .attr(format!("command(version = \"{}\")", info.version))
            .field(Field::new("command", "Commands").attr("command(subcommand)"));

        if let Some(desc) = &info.description {
            s = s.attr(format!("command(about = \"{}\")", desc));
        }

        fragments.push(CodeFragment::raw(s.build()));

        // Build dispatch impl
        let await_suffix = if info.is_async { ".await" } else { "" };
        let mut match_expr = Match::new("self.command");

        for cmd in &info.commands {
            let (pattern, body) = if cmd.has_subcommands {
                (
                    format!("Commands::{}(cmd)", cmd.pascal_name),
                    format!("cmd.dispatch(ctx){}", await_suffix),
                )
            } else {
                (
                    format!("Commands::{}(args)", cmd.pascal_name),
                    format!(
                        "crate::handlers::{}::run(ctx, args){}",
                        cmd.snake_name, await_suffix
                    ),
                )
            };
            match_expr = match_expr.arm(Arm::new(pattern).body(body));
        }

        let mut dispatch = Fn::new("dispatch")
            .param(Param::new("self", ""))
            .param(Param::new("ctx", "&Context"))
            .returns("eyre::Result<()>")
            .body_match(&match_expr);

        if info.is_async {
            dispatch = dispatch.async_();
        }

        fragments.push(CodeFragment::raw(Impl::new("Cli").method(dispatch).build()));

        // Build commands enum
        let mut e = Enum::new("Commands").derive("Subcommand").derive("Debug");

        for cmd in &info.commands {
            let data = if cmd.has_subcommands {
                cmd.pascal_name.clone()
            } else {
                format!("{}Args", cmd.pascal_name)
            };
            e = e.variant(
                Variant::new(&cmd.pascal_name)
                    .doc(&cmd.description)
                    .tuple(data),
            );
        }

        fragments.push(CodeFragment::raw(e.build()));

        fragments
    }

    fn generate_command(&self, info: &CommandMeta) -> Vec<CodeFragment> {
        let mut s = Struct::new(format!("{}Args", info.pascal_name))
            .doc(&info.description)
            .derive("Args")
            .derive("Debug");

        // Generate positional args
        for arg in &info.args {
            let rust_type = self.map_arg_type(arg.arg_type);
            let field_type = if arg.required && arg.default.is_none() {
                rust_type.to_string()
            } else {
                format!("Option<{}>", rust_type)
            };

            let mut field = Field::new(&arg.field_name, field_type);
            if let Some(desc) = &arg.description {
                field = field.doc(desc);
            }
            s = s.field(field);
        }

        // Generate flags
        for flag in &info.flags {
            let mut attrs = vec!["long".to_string()];
            if let Some(short) = flag.short {
                attrs.push(format!("short = '{}'", short));
            }
            if let Some(default) = &flag.default {
                attrs.push(format!("default_value = \"{}\"", default));
            }

            let rust_type = self.map_arg_type(flag.flag_type);
            let field_type = if flag.flag_type == ArgType::Bool {
                "bool".to_string()
            } else if flag.default.is_some() {
                rust_type.to_string()
            } else {
                format!("Option<{}>", rust_type)
            };

            let mut field =
                Field::new(&flag.field_name, field_type).attr(format!("arg({})", attrs.join(", ")));
            if let Some(desc) = &flag.description {
                field = field.doc(desc);
            }
            s = s.field(field);
        }

        vec![CodeFragment::raw(s.build())]
    }

    fn generate_subcommands(&self, info: &CommandMeta) -> Vec<CodeFragment> {
        let mut fragments = Vec::new();

        // Parent struct with subcommand field
        let parent_struct = Struct::new(&info.pascal_name)
            .doc(&info.description)
            .derive("Args")
            .derive("Debug")
            .field(
                Field::new("command", format!("{}Commands", info.pascal_name))
                    .attr("command(subcommand)"),
            );

        fragments.push(CodeFragment::raw(parent_struct.build()));

        // Subcommands enum
        let mut commands_enum = Enum::new(format!("{}Commands", info.pascal_name))
            .derive("Subcommand")
            .derive("Debug");

        for sub in &info.subcommands {
            let data = if sub.has_subcommands {
                sub.pascal_name.clone()
            } else {
                format!("{}Args", sub.pascal_name)
            };
            commands_enum = commands_enum.variant(
                Variant::new(&sub.pascal_name)
                    .doc(&sub.description)
                    .tuple(data),
            );
        }

        fragments.push(CodeFragment::raw(commands_enum.build()));

        fragments
    }

    fn generate_dispatch(&self, info: &DispatchInfo) -> Vec<CodeFragment> {
        let await_suffix = if info.is_async { ".await" } else { "" };

        let mut match_expr = Match::new("self.command");
        for sub in &info.subcommands {
            let (pattern, body) = if sub.has_subcommands {
                (
                    format!("{}Commands::{}(cmd)", info.parent_name, sub.pascal_name),
                    format!("cmd.dispatch(ctx){}", await_suffix),
                )
            } else {
                (
                    format!("{}Commands::{}(args)", info.parent_name, sub.pascal_name),
                    format!(
                        "crate::handlers::{}::{}::run(ctx, args){}",
                        info.handler_path, sub.snake_name, await_suffix
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

        if info.is_async {
            dispatch = dispatch.async_();
        }

        vec![CodeFragment::raw(
            Impl::new(&info.parent_name).method(dispatch).build(),
        )]
    }

    fn imports(&self) -> Vec<ImportSpec> {
        vec![ImportSpec::new("clap").symbols(["Parser", "Subcommand"])]
    }

    fn command_imports(&self, info: &CommandMeta) -> Vec<ImportSpec> {
        let mut imports = vec![ImportSpec::new("clap").symbol("Args")];

        if info.has_subcommands {
            imports.push(ImportSpec::new("clap").symbol("Subcommand"));
            imports.push(ImportSpec::new("crate::context").symbol("Context"));
        }

        imports
    }

    fn map_arg_type(&self, arg_type: ArgType) -> &'static str {
        match arg_type {
            ArgType::String => "String",
            ArgType::Int => "i64",
            ArgType::Float => "f64",
            ArgType::Bool => "bool",
            ArgType::Path => "std::path::PathBuf",
        }
    }

    fn map_optional_type(&self, arg_type: ArgType) -> String {
        format!("Option<{}>", self.map_arg_type(arg_type))
    }
}
