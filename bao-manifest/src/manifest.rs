use std::{collections::HashMap, path::Path, str::FromStr};

use serde::Deserialize;

use crate::{CliConfig, Command, Context, Error, Result, validate::ParseContext};

/// Root manifest for bao.toml
#[derive(Debug, Deserialize)]
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

impl FromStr for Manifest {
    type Err = Box<Error>;

    fn from_str(s: &str) -> Result<Self> {
        Self::from_str_with_filename(s, "bao.toml")
    }
}

impl Manifest {
    /// Parse a bao.toml file from the given path
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|e| {
            Box::new(Error::Io {
                path: path.to_path_buf(),
                source: e,
            })
        })?;
        Self::from_str_with_filename(&content, &path.display().to_string())
    }

    /// Parse a bao.toml from a string with a custom filename for error reporting
    pub fn from_str_with_filename(content: &str, filename: &str) -> Result<Self> {
        let manifest: Self =
            toml::from_str(content).map_err(|e| Error::parse(e, content, filename))?;
        manifest.validate(content, filename)?;
        Ok(manifest)
    }

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

    /// Validate the manifest after parsing
    fn validate(&self, src: &str, filename: &str) -> Result<()> {
        let ctx = ParseContext::new(src, filename);

        for (name, command) in &self.commands {
            ctx.validate_name(name, "command")?;

            // Create a context with the command name for nested validation
            let cmd_ctx = ctx.push(name);
            command.validate(&cmd_ctx)?;
        }
        Ok(())
    }
}
