use std::path::{Path, PathBuf};

use baobao_core::{FileRules, GeneratedFile, Overwrite, to_snake_case};

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
        // Convert module names to snake_case for valid Rust identifiers
        // (handles dashed names like "my-command" -> "my_command")
        for name in &self.modules {
            let module_name = to_snake_case(name);
            out.push_str(&format!("pub mod {};\n", module_name));
        }
        out
    }
}
