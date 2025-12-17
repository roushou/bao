//! Boune CLI framework adapter for TypeScript/Bun.

use baobao_codegen::{
    adapters::{CliAdapter, CliInfo, CommandMeta, Dependency, DispatchInfo, ImportSpec},
    builder::CodeFragment,
};
use baobao_core::ArgType;
use baobao_manifest::ArgType as ManifestArgType;

use crate::ast::{ArrowFn, JsObject, MethodChain};

/// Boune adapter for generating TypeScript CLI code targeting Bun runtime.
#[derive(Debug, Clone, Default)]
pub struct BouneAdapter;

impl BouneAdapter {
    pub fn new() -> Self {
        Self
    }

    /// Convert manifest ArgType to core ArgType.
    fn convert_arg_type(arg_type: &ManifestArgType) -> ArgType {
        match arg_type {
            ManifestArgType::String => ArgType::String,
            ManifestArgType::Int => ArgType::Int,
            ManifestArgType::Float => ArgType::Float,
            ManifestArgType::Bool => ArgType::Bool,
            ManifestArgType::Path => ArgType::Path,
        }
    }

    /// Build an argument chain for boune's declarative API using manifest types.
    pub fn build_argument_chain_manifest(
        &self,
        arg_type: &ManifestArgType,
        required: bool,
        has_default: bool,
        default: Option<&toml::Value>,
        description: Option<&str>,
        choices: Option<&[String]>,
    ) -> String {
        self.build_argument_chain(
            Self::convert_arg_type(arg_type),
            required,
            has_default,
            default,
            description,
            choices,
        )
    }

    /// Build an argument chain for boune's declarative API.
    pub fn build_argument_chain(
        &self,
        arg_type: ArgType,
        required: bool,
        has_default: bool,
        default: Option<&toml::Value>,
        description: Option<&str>,
        choices: Option<&[String]>,
    ) -> String {
        let boune_type = self.map_arg_type(arg_type);
        let mut chain = MethodChain::new(format!("argument.{}", boune_type));

        if required && !has_default {
            chain = chain.call_empty("required");
        }

        if let Some(choices) = choices {
            let choices_array = choices
                .iter()
                .map(|c| format!("\"{}\"", c))
                .collect::<Vec<_>>()
                .join(", ");
            chain = chain.call("choices", format!("[{}]", choices_array));
        }

        if let Some(default) = default {
            chain = chain.call("default", toml_to_ts_literal(default));
        }

        if let Some(desc) = description {
            chain = chain.call("describe", format!("\"{}\"", desc));
        }

        chain.build_inline()
    }

    /// Build an option chain for boune's declarative API using manifest types.
    pub fn build_option_chain_manifest(
        &self,
        flag_type: &ManifestArgType,
        short: Option<char>,
        default: Option<&toml::Value>,
        description: Option<&str>,
        choices: Option<&[String]>,
    ) -> String {
        self.build_option_chain(
            Self::convert_arg_type(flag_type),
            short,
            default,
            description,
            choices,
        )
    }

    /// Build an option chain for boune's declarative API.
    pub fn build_option_chain(
        &self,
        flag_type: ArgType,
        short: Option<char>,
        default: Option<&toml::Value>,
        description: Option<&str>,
        choices: Option<&[String]>,
    ) -> String {
        let boune_type = self.map_arg_type(flag_type);
        let mut chain = MethodChain::new(format!("option.{}", boune_type));

        if let Some(short) = short {
            chain = chain.call("short", format!("\"{}\"", short));
        }

        if let Some(choices) = choices {
            let choices_array = choices
                .iter()
                .map(|c| format!("\"{}\"", c))
                .collect::<Vec<_>>()
                .join(", ");
            chain = chain.call("choices", format!("[{}]", choices_array));
        }

        if let Some(default) = default {
            chain = chain.call("default", toml_to_ts_literal(default));
        }

        if let Some(desc) = description {
            chain = chain.call("describe", format!("\"{}\"", desc));
        }

        chain.build_inline()
    }

    /// Map manifest argument type to TypeScript boune type.
    pub fn map_manifest_arg_type(&self, arg_type: &ManifestArgType) -> &'static str {
        self.map_arg_type(Self::convert_arg_type(arg_type))
    }

    /// Build action handler arrow function.
    pub fn build_action_handler(&self, has_args: bool, has_options: bool) -> ArrowFn {
        // Build destructuring pattern based on what's available
        let params = match (has_args, has_options) {
            (true, true) => "{ args, options }",
            (true, false) => "{ args }",
            (false, true) => "{ options }",
            (false, false) => "{}",
        };

        // Build run() call based on what's available
        let run_call = match (has_args, has_options) {
            (true, true) => "await run(args, options);",
            (true, false) => "await run(args);",
            (false, true) => "await run(options);",
            (false, false) => "await run();",
        };

        ArrowFn::new(params).async_().body_line(run_call)
    }
}

impl CliAdapter for BouneAdapter {
    fn name(&self) -> &'static str {
        "boune"
    }

    fn dependencies(&self) -> Vec<Dependency> {
        vec![Dependency::new("boune", "^0.5.0")]
    }

    fn generate_cli(&self, info: &CliInfo) -> Vec<CodeFragment> {
        // Build CLI schema object
        let mut schema = JsObject::new()
            .string("name", &info.name)
            .string("version", info.version.to_string());

        if let Some(desc) = &info.description {
            schema = schema.string("description", desc);
        }

        // Build commands object
        let mut commands_obj = JsObject::new();
        for cmd in &info.commands {
            commands_obj =
                commands_obj.raw(&cmd.pascal_name, format!("{}Command", cmd.pascal_name));
        }
        schema = schema.object("commands", commands_obj);

        // Generate the defineCli call
        let code = format!("const app = defineCli({});", schema.build());
        vec![CodeFragment::raw(code)]
    }

    fn generate_command(&self, info: &CommandMeta) -> Vec<CodeFragment> {
        // This generates a leaf command definition
        let action = self.build_action_handler(!info.args.is_empty(), !info.flags.is_empty());

        let schema = JsObject::new()
            .string("name", &info.name)
            .string("description", &info.description)
            .raw_if(!info.args.is_empty(), "arguments", "args")
            .raw_if(!info.flags.is_empty(), "options", "options")
            .arrow_fn("action", action);

        let code = format!(
            "export const {}Command = defineCommand({});",
            info.pascal_name,
            schema.build()
        );

        vec![CodeFragment::raw(code)]
    }

    fn generate_subcommands(&self, info: &CommandMeta) -> Vec<CodeFragment> {
        // Build subcommands object
        let mut subcommands = JsObject::new();
        for sub in &info.subcommands {
            subcommands = subcommands.raw(&sub.pascal_name, format!("{}Command", sub.pascal_name));
        }

        let schema = JsObject::new()
            .string("name", &info.name)
            .string("description", &info.description)
            .object("subcommands", subcommands);

        let code = format!(
            "export const {}Command = defineCommand({});",
            info.pascal_name,
            schema.build()
        );

        vec![CodeFragment::raw(code)]
    }

    fn generate_dispatch(&self, _info: &DispatchInfo) -> Vec<CodeFragment> {
        // Boune handles dispatch internally via subcommands object
        // No explicit dispatch code needed
        Vec::new()
    }

    fn imports(&self) -> Vec<ImportSpec> {
        vec![ImportSpec::new("boune").symbol("defineCli")]
    }

    fn command_imports(&self, info: &CommandMeta) -> Vec<ImportSpec> {
        let mut imports = vec![ImportSpec::new("boune").symbol("defineCommand")];

        if !info.args.is_empty() {
            imports[0].symbols.push("argument".to_string());
            imports.push(ImportSpec::new("boune").symbol("InferArgs").type_only());
        }

        if !info.flags.is_empty() {
            imports[0].symbols.push("option".to_string());
            imports.push(ImportSpec::new("boune").symbol("InferOptions").type_only());
        }

        imports
    }

    fn map_arg_type(&self, arg_type: ArgType) -> &'static str {
        match arg_type {
            ArgType::String => "string",
            ArgType::Int => "number",
            ArgType::Float => "number",
            ArgType::Bool => "boolean",
            ArgType::Path => "string",
        }
    }

    fn map_optional_type(&self, arg_type: ArgType) -> String {
        format!("{} | undefined", self.map_arg_type(arg_type))
    }
}

/// Convert a TOML value to a TypeScript literal string.
/// Strings are quoted, numbers and booleans are raw.
fn toml_to_ts_literal(value: &toml::Value) -> String {
    match value {
        toml::Value::String(s) => format!("\"{}\"", s),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        _ => String::new(),
    }
}
