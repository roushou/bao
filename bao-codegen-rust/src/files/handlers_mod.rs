use std::path::{Path, PathBuf};

use baobao_core::{FileRules, GeneratedFile, to_snake_case};

use crate::{RawCode, RustFile};

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
        FileRules::always_overwrite()
    }

    fn render(&self) -> String {
        // Convert module names to snake_case for valid Rust identifiers
        // (handles dashed names like "my-command" -> "my_command")
        let mods: Vec<String> = self
            .modules
            .iter()
            .map(|name| format!("pub mod {};", to_snake_case(name)))
            .collect();

        RustFile::new().add(RawCode::lines(mods)).render()
    }
}
