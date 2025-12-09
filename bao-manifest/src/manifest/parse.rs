//! Manifest parsing from files and strings.

use std::{path::Path, str::FromStr};

use super::{Manifest, validate::ParseContext};
use crate::{Error, Result, error::SourceContext};

impl FromStr for Manifest {
    type Err = Box<Error>;

    fn from_str(s: &str) -> Result<Self> {
        parse_manifest(s, "bao.toml")
    }
}

impl Manifest {
    /// Parse a bao.toml file from the given path.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|e| {
            Box::new(Error::Io {
                path: path.to_path_buf(),
                source: e,
            })
        })?;
        parse_manifest(&content, &path.display().to_string())
    }

    /// Parse a bao.toml from a string with a custom filename for error reporting.
    pub fn from_str_with_filename(content: &str, filename: &str) -> Result<Self> {
        parse_manifest(content, filename)
    }
}

/// Parse a manifest from content with the given filename for error reporting.
pub fn parse_manifest(content: &str, filename: &str) -> Result<Manifest> {
    let source_ctx = SourceContext::new(content, filename);
    let manifest: Manifest = toml::from_str(content).map_err(|e| source_ctx.parse_error(e))?;
    validate_manifest(&manifest, content, filename)?;
    Ok(manifest)
}

/// Validate the manifest after parsing.
fn validate_manifest(manifest: &Manifest, src: &str, filename: &str) -> Result<()> {
    let ctx = ParseContext::new(src, filename);

    for (name, command) in &manifest.commands {
        ctx.validate_name(name, "command")?;

        // Create a context with the command name for nested validation
        let cmd_ctx = ctx.push(name);
        command.validate(&cmd_ctx)?;
    }
    Ok(())
}
