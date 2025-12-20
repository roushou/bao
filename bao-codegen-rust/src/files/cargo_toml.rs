use std::path::{Path, PathBuf};

use baobao_core::{FileRules, GeneratedFile, Version};

const DEFAULT_EDITION: &str = "2024";

/// The Cargo.toml project manifest
pub struct CargoToml {
    pub name: String,
    pub version: Version,
    pub edition: String,
    pub dependencies: Vec<(String, String)>,
}

impl CargoToml {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: Version::new(0, 1, 0),
            edition: DEFAULT_EDITION.to_string(),
            dependencies: Vec::new(),
        }
    }

    pub fn with_version(mut self, version: Version) -> Self {
        self.version = version;
        self
    }

    pub fn with_edition(mut self, edition: impl Into<String>) -> Self {
        self.edition = edition.into();
        self
    }

    pub fn with_dependency(mut self, dependency: (String, String)) -> Self {
        self.dependencies.push(dependency);
        self
    }

    pub fn with_dependencies(mut self, dependencies: Vec<(String, String)>) -> Self {
        self.dependencies = dependencies;
        self
    }
}

impl GeneratedFile for CargoToml {
    fn path(&self, base: &Path) -> PathBuf {
        base.join("Cargo.toml")
    }

    fn rules(&self) -> FileRules {
        FileRules::always_overwrite()
    }

    fn render(&self) -> String {
        let mut out = format!(
            r#"[package]
name = "{}"
version = "{}"
edition = "{}"

[dependencies]
"#,
            self.name, self.version, self.edition
        );

        for (dep_name, dep_version) in &self.dependencies {
            if dep_version.contains('{') {
                // Complex dependency with features
                out.push_str(&format!("{} = {}\n", dep_name, dep_version));
            } else {
                out.push_str(&format!("{} = \"{}\"\n", dep_name, dep_version));
            }
        }

        out
    }
}
