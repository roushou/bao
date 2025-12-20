//! .gitignore generator for TypeScript/Bun projects.

use std::path::{Path, PathBuf};

use baobao_core::{FileRules, GeneratedFile};

/// The .gitignore file for Node.js/Bun projects.
pub struct GitIgnore;

impl GeneratedFile for GitIgnore {
    fn path(&self, base: &Path) -> PathBuf {
        base.join(".gitignore")
    }

    fn rules(&self) -> FileRules {
        FileRules::create_once()
    }

    fn render(&self) -> String {
        r#"# Dependencies
node_modules/

# Build output
dist/

# Bun
bun.lockb

# Environment
.env
.env.local
.env.*.local

# IDE
.idea/
.vscode/
*.swp
*.swo

# OS
.DS_Store
Thumbs.db

# Debug
*.log
"#
        .to_string()
    }
}
