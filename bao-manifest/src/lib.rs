// Miette's derive macro generates code that triggers these warnings
#![allow(unused_assignments)]

mod cli;
mod command;
mod context;
mod error;
mod file;
mod validate;

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
    /// Only [context.database] and [context.http] are allowed
    #[serde(default, deserialize_with = "deserialize_context")]
    pub context: Context,

    /// Top-level commands
    #[serde(default)]
    pub commands: HashMap<String, Command>,
}

fn deserialize_context<'de, D>(deserializer: D) -> std::result::Result<Context, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use context::DatabaseContextField;
    use serde::de::Error;

    #[derive(Deserialize)]
    struct RawContext {
        database: Option<toml::Value>,
        http: Option<toml::Value>,
    }

    let raw: RawContext = RawContext::deserialize(deserializer)?;
    let mut ctx = Context::default();

    if let Some(db_value) = raw.database {
        let db: DatabaseContextField = db_value
            .try_into()
            .map_err(|e: toml::de::Error| D::Error::custom(e.message()))?;
        ctx.database = Some(db.into());
    }

    if let Some(http_value) = raw.http {
        let http: HttpConfig = http_value
            .try_into()
            .map_err(|e: toml::de::Error| D::Error::custom(e.message()))?;
        ctx.http = Some(http);
    }

    Ok(ctx)
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
            // Validate command name is a valid Rust identifier
            validate_name(name, "command", src, filename)?;
            command.validate(name, src, filename)?;
        }
        Ok(())
    }
}

/// Validate that a name is a valid Rust identifier
fn validate_name(name: &str, context: &str, src: &str, filename: &str) -> Result<()> {
    let span = validate::find_name_span(src, name);

    if validate::is_rust_keyword(name) {
        return Err(Error::reserved_keyword(name, context, src, filename, span));
    }

    if let Some(reason) = validate::validate_identifier(name) {
        return Err(Error::invalid_identifier(
            name, context, reason, src, filename, span,
        ));
    }

    Ok(())
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
