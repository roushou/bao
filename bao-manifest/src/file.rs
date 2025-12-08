use std::path::{Path, PathBuf};

use crate::{Result, Schema, parse_str_with_filename};

/// Represents a bao.toml file with both raw content and parsed schema.
pub struct BaoToml {
    path: PathBuf,
    content: String,
    schema: Schema,
}

impl BaoToml {
    /// Open and parse a bao.toml file.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let content = std::fs::read_to_string(&path).map_err(|e| {
            Box::new(crate::Error::Io {
                path: path.clone(),
                source: e,
            })
        })?;
        let filename = path.display().to_string();
        let schema = parse_str_with_filename(&content, &filename)?;

        Ok(Self {
            path,
            content,
            schema,
        })
    }

    /// Get the file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the raw content.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Get the parsed schema.
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// Update content and re-parse the schema.
    pub fn set_content(&mut self, content: String) -> Result<()> {
        let filename = self.path.display().to_string();
        let schema = parse_str_with_filename(&content, &filename)?;
        self.content = content;
        self.schema = schema;
        Ok(())
    }

    /// Save the current content to disk.
    pub fn save(&self) -> Result<()> {
        std::fs::write(&self.path, &self.content).map_err(|e| {
            Box::new(crate::Error::Io {
                path: self.path.clone(),
                source: e,
            })
        })?;
        Ok(())
    }
}
