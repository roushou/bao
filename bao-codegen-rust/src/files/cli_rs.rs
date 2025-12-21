use std::path::{Path, PathBuf};

use baobao_codegen::schema::CommandInfo;
use baobao_core::{FileRules, GeneratedFile, Version, to_pascal_case, to_snake_case};

use super::{GENERATED_HEADER, uses};
use crate::{Arm, ClapAttr, Enum, Field, Fn, Impl, Match, Param, RustFile, Struct, Use, Variant};

/// The cli.rs file containing the main CLI struct and dispatch logic
pub struct CliRs {
    pub name: String,
    pub version: Version,
    pub description: Option<String>,
    pub commands: Vec<CommandInfo>,
    pub is_async: bool,
}

impl CliRs {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        description: Option<String>,
        commands: Vec<CommandInfo>,
        is_async: bool,
    ) -> Self {
        let version_str = version.into();
        Self {
            name: name.into(),
            version: version_str
                .parse()
                .unwrap_or_else(|_| Version::new(0, 1, 0)),
            description,
            commands,
            is_async,
        }
    }

    /// Create with a parsed Version (for backwards compatibility).
    pub fn with_version(
        name: impl Into<String>,
        version: Version,
        description: Option<String>,
        commands: Vec<CommandInfo>,
        is_async: bool,
    ) -> Self {
        Self {
            name: name.into(),
            version,
            description,
            commands,
            is_async,
        }
    }

    fn build_cli_struct(&self) -> Struct {
        Struct::new("Cli")
            .derive("Parser")
            .derive("Debug")
            .clap_attr(ClapAttr::command_name(&self.name))
            .clap_attr(ClapAttr::command_version(self.version.to_string()))
            .clap_attr_if(
                self.description.is_some(),
                ClapAttr::command_about(self.description.as_deref().unwrap_or("")),
            )
            .field(Field::new("command", "Commands").clap_attr(ClapAttr::command_subcommand()))
    }

    fn build_dispatch_impl(&self) -> Impl {
        let await_suffix = if self.is_async { ".await" } else { "" };

        let mut match_expr = Match::new("self.command");
        for cmd in &self.commands {
            let pascal = to_pascal_case(&cmd.name);
            let (pattern, body) = if cmd.has_subcommands {
                (
                    format!("Commands::{}(cmd)", pascal),
                    format!("cmd.dispatch(ctx){}", await_suffix),
                )
            } else {
                // Use snake_case for module paths (handles dashed names like "my-command" -> "my_command")
                let module_name = to_snake_case(&cmd.name);
                (
                    format!("Commands::{}(args)", pascal),
                    format!(
                        "crate::handlers::{}::run(ctx, args){}",
                        module_name, await_suffix
                    ),
                )
            };
            match_expr = match_expr.arm(Arm::new(pattern).body(body));
        }

        let dispatch = Fn::new("dispatch")
            .param(Param::new("self", ""))
            .param(Param::new("ctx", "&Context"))
            .returns("eyre::Result<()>")
            .body_match(&match_expr)
            .async_if(self.is_async);

        Impl::new("Cli").method(dispatch)
    }

    fn build_commands_enum(&self) -> Enum {
        let mut e = Enum::new("Commands").derive("Subcommand").derive("Debug");

        for cmd in &self.commands {
            let pascal = to_pascal_case(&cmd.name);
            let data = if cmd.has_subcommands {
                pascal.clone()
            } else {
                format!("{}Args", pascal)
            };
            e = e.variant(Variant::new(&pascal).doc(&cmd.description).tuple(data));
        }

        e
    }
}

impl GeneratedFile for CliRs {
    fn path(&self, base: &Path) -> PathBuf {
        base.join("src").join("generated").join("cli.rs")
    }

    fn rules(&self) -> FileRules {
        FileRules::always_overwrite().with_header(GENERATED_HEADER)
    }

    fn render(&self) -> String {
        RustFile::new()
            .use_stmt(uses::clap_parser_subcommand())
            .use_stmt(Use::new("super::commands").symbol("*"))
            .use_stmt(uses::context())
            .add(self.build_cli_struct())
            .add(self.build_dispatch_impl())
            .add(self.build_commands_enum())
            .render_with_header(GENERATED_HEADER)
    }
}
