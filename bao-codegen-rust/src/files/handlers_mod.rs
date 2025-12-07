use std::path::{Path, PathBuf};

use baobao_core::{FileRules, GeneratedFile, Overwrite};

/// The handlers/mod.rs file that exports all handler modules
pub struct HandlersMod {
    pub modules: Vec<String>,
}

impl HandlersMod {
    pub fn new(modules: Vec<String>) -> Self {
        Self { modules }
    }
}

impl GeneratedFile for HandlersMod {
    fn path(&self, base: &Path) -> PathBuf {
        base.join("src").join("handlers").join("mod.rs")
    }

    fn rules(&self) -> FileRules {
        FileRules {
            overwrite: Overwrite::Always,
            header: None,
        }
    }

    fn render(&self) -> String {
        let mut out = String::new();
        for name in &self.modules {
            out.push_str(&format!("pub mod {};\n", name));
        }
        out
    }
}
