//! Shared bao.toml generator.

use std::path::{Path, PathBuf};

use baobao_core::{FileRules, GeneratedFile, Overwrite, Version};
use baobao_manifest::Language;

/// The bao.toml configuration file.
pub struct BaoToml {
    pub name: String,
    pub version: Version,
    pub description: String,
    pub language: Language,
    pub overwrite: Overwrite,
}

impl BaoToml {
    pub fn new(name: impl Into<String>, language: Language) -> Self {
        Self {
            name: name.into(),
            version: Version::new(0, 1, 0),
            description: "A CLI application".to_string(),
            language,
            overwrite: Overwrite::IfMissing,
        }
    }

    pub fn with_version(mut self, version: Version) -> Self {
        self.version = version;
        self
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = description;
        self
    }

    pub fn with_overwrite(mut self, overwrite: Overwrite) -> Self {
        self.overwrite = overwrite;
        self
    }
}

impl GeneratedFile for BaoToml {
    fn path(&self, base: &Path) -> PathBuf {
        base.join("bao.toml")
    }

    fn rules(&self) -> FileRules {
        FileRules {
            overwrite: self.overwrite,
            header: None,
        }
    }

    fn render(&self) -> String {
        format!(
            r#"[cli]
name = "{}"
version = "{}"
description = "{}"
language = "{}"

# Uncomment to add shared resources accessible in all handlers:
# [context.database]
# type = "sqlite"
# env = "DATABASE_URL"
# create_if_missing = true
# journal_mode = "wal"
# synchronous = "normal"
# busy_timeout = 5000
# foreign_keys = true
# max_connections = 5
#
# [context.http]
# type = "http"
#
# Supported types: sqlite, postgres, mysql, http

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
            self.name, self.version, self.description, self.language
        )
    }
}
