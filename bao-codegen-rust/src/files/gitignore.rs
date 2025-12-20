use std::path::{Path, PathBuf};

use baobao_core::{FileRules, GeneratedFile};

/// The .gitignore file
pub struct GitIgnore;

impl GeneratedFile for GitIgnore {
    fn path(&self, base: &Path) -> PathBuf {
        base.join(".gitignore")
    }

    fn rules(&self) -> FileRules {
        FileRules::create_once()
    }

    fn render(&self) -> String {
        "/target\n".to_string()
    }
}
