//! package.json generator for TypeScript projects.

use std::path::{Path, PathBuf};

use baobao_core::{FileRules, GeneratedFile, Overwrite};

const DEFAULT_VERSION: &str = "0.1.0";
const DEFAULT_DESCRIPTION: &str = "A CLI application";

/// The package.json configuration file.
pub struct PackageJson {
    pub name: String,
    pub version: String,
    pub description: String,
    pub dependencies: Vec<Dependency>,
    pub dev_dependencies: Vec<Dependency>,
}

impl PackageJson {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: DEFAULT_VERSION.to_string(),
            description: DEFAULT_DESCRIPTION.to_string(),
            dependencies: vec![Dependency::new("boune", "^0.2.0")],
            dev_dependencies: vec![
                Dependency::new("@types/bun", "latest"),
                Dependency::new("typescript", "^5.0.0"),
            ],
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

    pub fn with_dependency(mut self, dep: impl Into<Dependency>) -> Self {
        self.dependencies.push(dep.into());
        self
    }

    pub fn with_dependencies(
        mut self,
        deps: impl IntoIterator<Item = impl Into<Dependency>>,
    ) -> Self {
        self.dependencies.extend(deps.into_iter().map(Into::into));
        self
    }

    pub fn with_dev_dependency(mut self, dep: impl Into<Dependency>) -> Self {
        self.dev_dependencies.push(dep.into());
        self
    }

    pub fn with_dev_dependencies(
        mut self,
        deps: impl IntoIterator<Item = impl Into<Dependency>>,
    ) -> Self {
        self.dev_dependencies
            .extend(deps.into_iter().map(Into::into));
        self
    }

    fn render_dependencies(deps: &[Dependency]) -> String {
        deps.iter()
            .map(|d| format!("    \"{}\": \"{}\"", d.name, d.version))
            .collect::<Vec<_>>()
            .join(",\n")
    }
}

impl GeneratedFile for PackageJson {
    fn path(&self, base: &Path) -> PathBuf {
        base.join("package.json")
    }

    fn rules(&self) -> FileRules {
        FileRules {
            overwrite: Overwrite::IfMissing,
            header: None,
        }
    }

    fn render(&self) -> String {
        let dependencies = Self::render_dependencies(&self.dependencies);
        let dev_dependencies = Self::render_dependencies(&self.dev_dependencies);

        format!(
            r#"{{
  "name": "{}",
  "version": "{}",
  "description": "{}",
  "type": "module",
  "scripts": {{
    "dev": "bun run src/index.ts",
    "build": "bun build src/index.ts --outdir dist --target bun",
    "start": "bun run dist/index.js"
  }},
  "dependencies": {{
{}
  }},
  "devDependencies": {{
{}
  }}
}}
"#,
            self.name, self.version, self.description, dependencies, dev_dependencies
        )
    }
}

/// A dependency with name and version.
#[derive(Debug, Clone)]
pub struct Dependency {
    name: String,
    version: String,
}

impl Dependency {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
        }
    }
}

impl<N: Into<String>, V: Into<String>> From<(N, V)> for Dependency {
    fn from((name, version): (N, V)) -> Self {
        Self::new(name, version)
    }
}
