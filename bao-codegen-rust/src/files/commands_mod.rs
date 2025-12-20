use std::path::{Path, PathBuf};

use baobao_core::{FileRules, GeneratedFile, to_snake_case};

use super::GENERATED_HEADER;
use crate::{RawCode, RustFile};

/// The commands/mod.rs file that exports all command modules
pub struct CommandsMod {
    pub commands: Vec<String>,
}

impl CommandsMod {
    pub fn new(commands: Vec<String>) -> Self {
        Self { commands }
    }
}

impl GeneratedFile for CommandsMod {
    fn path(&self, base: &Path) -> PathBuf {
        base.join("src")
            .join("generated")
            .join("commands")
            .join("mod.rs")
    }

    fn rules(&self) -> FileRules {
        FileRules::always_overwrite().with_header(GENERATED_HEADER)
    }

    fn render(&self) -> String {
        // Convert command names to snake_case for Rust module names
        // (handles dashed names like "my-command" -> "my_command")
        let mods: Vec<String> = self
            .commands
            .iter()
            .map(|name| format!("pub mod {};", to_snake_case(name)))
            .collect();

        let uses: Vec<String> = self
            .commands
            .iter()
            .map(|name| format!("pub use {}::*;", to_snake_case(name)))
            .collect();

        RustFile::new()
            .add(RawCode::lines(mods))
            .add(RawCode::lines(uses))
            .render_with_header(GENERATED_HEADER)
    }
}
