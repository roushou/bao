//! Boune CLI framework adapter for TypeScript/Bun.

use baobao_codegen::{
    adapters::{
        CliAdapter, CliInfo, CommandMeta, Dependency, DispatchInfo, ImportSpec,
        input_type_to_arg_type,
    },
    builder::CodeFragment,
};
use baobao_core::ArgType;
use baobao_ir::{Input, InputKind};
use baobao_manifest::ArgType as ManifestArgType;

use crate::{
    BOUNE_VERSION,
    ast::{ArrowFn, JsArray, JsObject},
};

/// Boune adapter for generating TypeScript CLI code targeting Bun runtime.
#[derive(Debug, Clone, Default)]
pub struct BouneAdapter;

impl BouneAdapter {
    pub fn new() -> Self {
        Self
    }

    /// Convert manifest ArgType to core ArgType.
    fn convert_manifest_arg_type(arg_type: &ManifestArgType) -> ArgType {
        match arg_type {
            ManifestArgType::String => ArgType::String,
            ManifestArgType::Int => ArgType::Int,
            ManifestArgType::Float => ArgType::Float,
            ManifestArgType::Bool => ArgType::Bool,
            ManifestArgType::Path => ArgType::Path,
        }
    }

    /// Build an argument object schema for boune's declarative API using manifest types.
    pub fn build_argument_schema_manifest(
        &self,
        arg_type: &ManifestArgType,
        required: bool,
        default: Option<&toml::Value>,
        description: Option<&str>,
        choices: Option<&[String]>,
    ) -> JsObject {
        self.build_argument_schema(
            Self::convert_manifest_arg_type(arg_type),
            required,
            default,
            description,
            choices,
        )
    }

    /// Build an argument object schema for boune's declarative API.
    ///
    /// Generates: `{ type: "string", required: true, description: "...", choices: [...] as const }`
    pub fn build_argument_schema(
        &self,
        arg_type: ArgType,
        required: bool,
        default: Option<&toml::Value>,
        description: Option<&str>,
        choices: Option<&[String]>,
    ) -> JsObject {
        let boune_type = self.map_arg_type(arg_type);

        JsObject::new()
            .string("type", boune_type)
            .raw_if(required && default.is_none(), "required", "true")
            .toml_opt("default", default)
            .string_opt("description", description)
            .array_opt(
                "choices",
                choices.map(|c| JsArray::from_strings(c).as_const()),
            )
    }

    /// Build an option object schema for boune's declarative API using manifest types.
    pub fn build_option_schema_manifest(
        &self,
        flag_type: &ManifestArgType,
        short: Option<char>,
        default: Option<&toml::Value>,
        description: Option<&str>,
        choices: Option<&[String]>,
    ) -> JsObject {
        self.build_option_schema(
            Self::convert_manifest_arg_type(flag_type),
            short,
            default,
            description,
            choices,
        )
    }

    /// Build an option object schema for boune's declarative API.
    ///
    /// Generates: `{ type: "string", short: "x", default: ..., description: "...", choices: [...] as const }`
    pub fn build_option_schema(
        &self,
        flag_type: ArgType,
        short: Option<char>,
        default: Option<&toml::Value>,
        description: Option<&str>,
        choices: Option<&[String]>,
    ) -> JsObject {
        let boune_type = self.map_arg_type(flag_type);

        JsObject::new()
            .string("type", boune_type)
            .string_opt("short", short.map(|c| c.to_string()))
            .toml_opt("default", default)
            .string_opt("description", description)
            .array_opt(
                "choices",
                choices.map(|c| JsArray::from_strings(c).as_const()),
            )
    }

    /// Map manifest argument type to TypeScript boune type.
    pub fn map_manifest_arg_type(&self, arg_type: &ManifestArgType) -> &'static str {
        self.map_arg_type(Self::convert_manifest_arg_type(arg_type))
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

    // ========================================================================
    // IR-based methods
    // ========================================================================

    /// Build an argument object schema from IR Input.
    pub fn build_argument_schema_ir(&self, input: &Input) -> JsObject {
        let boune_type = self.map_arg_type(input_type_to_arg_type(input.ty));

        JsObject::new()
            .string("type", boune_type)
            .raw_if(
                input.required && input.default.is_none(),
                "required",
                "true",
            )
            .default_value_opt("default", input.default.as_ref())
            .string_opt("description", input.description.as_deref())
            .array_opt(
                "choices",
                input
                    .choices
                    .as_ref()
                    .map(|c| JsArray::from_strings(c).as_const()),
            )
    }

    /// Build an option object schema from IR Input.
    pub fn build_option_schema_ir(&self, input: &Input) -> JsObject {
        let boune_type = self.map_arg_type(input_type_to_arg_type(input.ty));
        let short = if let InputKind::Flag { short } = &input.kind {
            *short
        } else {
            None
        };

        JsObject::new()
            .string("type", boune_type)
            .string_opt("short", short.map(|c| c.to_string()))
            .default_value_opt("default", input.default.as_ref())
            .string_opt("description", input.description.as_deref())
            .array_opt(
                "choices",
                input
                    .choices
                    .as_ref()
                    .map(|c| JsArray::from_strings(c).as_const()),
            )
    }
}

impl CliAdapter for BouneAdapter {
    fn name(&self) -> &'static str {
        "boune"
    }

    fn dependencies(&self) -> Vec<Dependency> {
        vec![Dependency::new("boune", BOUNE_VERSION)]
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

        // No longer need to import `argument` or `option` builders
        // Type inference helpers are still needed
        if !info.args.is_empty() {
            imports.push(ImportSpec::new("boune").symbol("InferArgs").type_only());
        }

        if !info.flags.is_empty() {
            imports.push(ImportSpec::new("boune").symbol("InferOpts").type_only());
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
