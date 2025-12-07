// Miette's derive macro generates code that triggers these warnings
#![allow(unused_assignments)]

mod cli;
mod command;
mod context;
mod error;
mod file;

use std::{collections::HashMap, path::Path};

pub use cli::*;
pub use command::*;
pub use context::*;
pub use error::{Error, Result};
pub use file::BaoToml;
use serde::Deserialize;

/// Root schema for bao.toml
#[derive(Debug, Deserialize)]
pub struct Schema {
    /// CLI metadata
    pub cli: CliConfig,

    /// Application context (shared resources)
    #[serde(default, deserialize_with = "deserialize_context")]
    pub context: HashMap<String, ContextField>,

    /// Top-level commands
    #[serde(default)]
    pub commands: HashMap<String, Command>,
}

fn deserialize_context<'de, D>(
    deserializer: D,
) -> std::result::Result<HashMap<String, ContextField>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use context::DatabaseContextField;
    use serde::de::Error;

    let raw: HashMap<String, toml::Value> = HashMap::deserialize(deserializer)?;
    let mut result = HashMap::new();

    for (key, value) in raw {
        let field = if key == "http" {
            // [context.http] is always an HTTP client
            let config: HttpConfig = value
                .try_into()
                .map_err(|e: toml::de::Error| D::Error::custom(e.message()))?;
            ContextField::Http(config)
        } else {
            // All other context fields must have a type (database types)
            let db: DatabaseContextField = value
                .try_into()
                .map_err(|e: toml::de::Error| D::Error::custom(e.message()))?;
            db.into()
        };
        result.insert(key, field);
    }

    Ok(result)
}

impl Schema {
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

    /// Validate the schema after parsing
    pub fn validate(&self, src: &str, filename: &str) -> Result<()> {
        for (name, command) in &self.commands {
            command.validate(name, src, filename)?;
        }
        Ok(())
    }
}

/// Parse a bao.toml file from the given path
pub fn parse_file(path: impl AsRef<Path>) -> Result<Schema> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path).map_err(|e| {
        Box::new(Error::Io {
            path: path.to_path_buf(),
            source: e,
        })
    })?;
    let filename = path.display().to_string();
    parse_str_with_filename(&content, &filename)
}

/// Parse a bao.toml from a string (uses "bao.toml" as default filename)
pub fn parse_str(content: &str) -> Result<Schema> {
    parse_str_with_filename(content, "bao.toml")
}

/// Parse a bao.toml from a string with a custom filename for error reporting
pub fn parse_str_with_filename(content: &str, filename: &str) -> Result<Schema> {
    let schema: Schema = toml::from_str(content).map_err(|e| Error::parse(e, content, filename))?;

    schema.validate(content, filename)?;
    Ok(schema)
}
