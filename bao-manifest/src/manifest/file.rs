use std::path::{Path, PathBuf};

use super::Manifest;
use crate::Result;

/// Represents a bao.toml file with both raw content and parsed manifest.
pub struct BaoToml {
    path: PathBuf,
    content: String,
    manifest: Manifest,
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
        let manifest = Manifest::from_str_with_filename(&content, &filename)?;

        Ok(Self {
            path,
            content,
            manifest,
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

    /// Get the parsed manifest.
    pub fn schema(&self) -> &Manifest {
        &self.manifest
    }

    /// Update content and re-parse the manifest.
    pub fn set_content(&mut self, content: String) -> Result<()> {
        let filename = self.path.display().to_string();
        let manifest = Manifest::from_str_with_filename(&content, &filename)?;
        self.content = content;
        self.manifest = manifest;
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
