use std::path::{Path, PathBuf};

use baobao_core::{FileRules, GeneratedFile, Overwrite};

/// The .gitignore file
pub struct GitIgnore;

impl GeneratedFile for GitIgnore {
    fn path(&self, base: &Path) -> PathBuf {
        base.join(".gitignore")
    }

    fn rules(&self) -> FileRules {
        FileRules {
            overwrite: Overwrite::IfMissing,
            header: None,
        }
    }

    fn render(&self) -> String {
        "/target\n".to_string()
    }
}
