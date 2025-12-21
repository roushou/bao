//! cli.ts generator for TypeScript projects using boune.

use std::path::{Path, PathBuf};

use baobao_codegen::schema::CommandInfo;
use baobao_core::{FileRules, GeneratedFile, Version, to_camel_case, to_kebab_case};

use super::GENERATED_HEADER;
use crate::{
    ast::{Const, Import, JsObject},
    code_file::{CodeFile, RawCode},
};

/// The cli.ts file containing the main CLI setup using boune.
pub struct CliTs {
    pub name: String,
    pub version: Version,
    pub description: Option<String>,
    pub commands: Vec<CommandInfo>,
}

impl CliTs {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        description: Option<String>,
        commands: Vec<CommandInfo>,
    ) -> Self {
        let version_str = version.into();
        Self {
            name: name.into(),
            version: version_str
                .parse()
                .unwrap_or_else(|_| Version::new(0, 1, 0)),
            description,
            commands,
        }
    }

    /// Create with a parsed Version (for backwards compatibility).
    pub fn with_version(
        name: impl Into<String>,
        version: Version,
        description: Option<String>,
        commands: Vec<CommandInfo>,
    ) -> Self {
        Self {
            name: name.into(),
            version,
            description,
            commands,
        }
    }

    fn build_imports(&self) -> Vec<Import> {
        let mut imports = vec![Import::new("boune").named("defineCli")];

        for cmd in &self.commands {
            let camel = to_camel_case(&cmd.name);
            let file = to_kebab_case(&cmd.name);
            imports.push(
                Import::new(format!("./commands/{}.ts", file)).named(format!("{}Command", camel)),
            );
        }

        imports
    }

    fn build_cli_schema(&self) -> String {
        // Build the commands object
        let commands = self.commands.iter().fold(JsObject::new(), |obj, cmd| {
            let camel = to_camel_case(&cmd.name);
            obj.raw(&camel, format!("{}Command", camel))
        });

        // Build the CLI config object
        let config = JsObject::new()
            .string("name", &self.name)
            .string("version", self.version.to_string())
            .string_opt("description", self.description.clone())
            .object("commands", commands);

        format!("defineCli({})", config.build().trim_end())
    }
}

impl GeneratedFile for CliTs {
    fn path(&self, base: &Path) -> PathBuf {
        base.join("src").join("cli.ts")
    }

    fn rules(&self) -> FileRules {
        FileRules::always_overwrite().with_header(GENERATED_HEADER)
    }

    fn render(&self) -> String {
        let file = CodeFile::new()
            .add(RawCode::new(GENERATED_HEADER))
            .imports(self.build_imports())
            .add(Const::new("app", self.build_cli_schema()));

        file.render()
    }
}
