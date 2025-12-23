//! Manifest types and parsing for bao.toml files.

mod cli;
mod edit;
mod file;
mod language;
mod parse;
mod validate;

use std::collections::HashMap;

pub use cli::CliConfig;
pub use edit::{
    append_section, command_section_header, context_section_header, remove_toml_section,
    rename_command_section,
};
pub use file::BaoToml;
pub use language::Language;
use serde::Deserialize;
pub use validate::ParseContext;

use crate::{Command, Context};

/// Root manifest for bao.toml
#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    /// CLI metadata
    pub cli: CliConfig,

    /// Application context (shared resources)
    /// Only [context.database] and [context.http] are allowed
    #[serde(default, deserialize_with = "crate::context::deserialize")]
    pub context: Context,

    /// Top-level commands
    #[serde(default)]
    pub commands: HashMap<String, Command>,
}

impl Manifest {
    /// Check if a command exists (supports nested paths like "users/create")
    pub fn has_command(&self, name: &str) -> bool {
        let parts: Vec<&str> = name.split('/').collect();

        if parts.len() == 1 {
            return self.commands.contains_key(name);
        }

        let mut current = &self.commands;
        for (i, part) in parts.iter().enumerate() {
            match current.get(*part) {
                Some(cmd) if i == parts.len() - 1 => return true,
                Some(cmd) => current = &cmd.commands,
                None => return false,
            }
        }
        false
    }
}
