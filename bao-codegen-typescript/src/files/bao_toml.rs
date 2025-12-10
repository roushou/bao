//! bao.toml generator for TypeScript projects.

use std::path::{Path, PathBuf};

use baobao_core::{FileRules, GeneratedFile, Overwrite};

/// The bao.toml configuration file.
pub struct BaoToml {
    pub name: String,
    pub version: String,
    pub description: String,
}

impl BaoToml {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: "0.1.0".to_string(),
            description: "A CLI application".to_string(),
        }
    }

    pub fn with_version(mut self, version: String) -> Self {
        self.version = version;
        self
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = description;
        self
    }
}

impl GeneratedFile for BaoToml {
    fn path(&self, base: &Path) -> PathBuf {
        base.join("bao.toml")
    }

    fn rules(&self) -> FileRules {
        FileRules {
            overwrite: Overwrite::IfMissing,
            header: None,
        }
    }

    fn render(&self) -> String {
        format!(
            r#"[cli]
name = "{}"
version = "{}"
description = "{}"
language = "typescript"

# Uncomment to add shared resources accessible in all handlers:
# [context.database]
# type = "sqlite"
# env = "DATABASE_URL"
# create_if_missing = true

[commands.hello]
description = "Say hello"

[[commands.hello.args]]
name = "name"
type = "string"
required = false
description = "Name to greet"

[[commands.hello.flags]]
name = "uppercase"
type = "bool"
short = "u"
description = "Print in uppercase"
"#,
            self.name, self.version, self.description
        )
    }
}
