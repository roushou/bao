use std::path::{Path, PathBuf};

use eyre::Result;

/// Trait for types that represent a generated file
pub trait GeneratedFile {
    /// Get the file path relative to the base directory
    fn path(&self, base: &Path) -> PathBuf;

    /// Get the rules for writing this file
    fn rules(&self) -> FileRules;

    /// Render the file content
    fn render(&self) -> String;

    /// Write the file to disk
    fn write(&self, base: &Path) -> Result<WriteResult> {
        let path = self.path(base);
        let rules = self.rules();

        match rules.overwrite {
            Overwrite::Always => {
                write_file(&path, &self.render())?;
                Ok(WriteResult::Written)
            }
            Overwrite::IfMissing => {
                if path.exists() {
                    Ok(WriteResult::Skipped)
                } else {
                    write_file(&path, &self.render())?;
                    Ok(WriteResult::Written)
                }
            }
        }
    }
}

fn write_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}

/// Result of a write operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteResult {
    /// File was written
    Written,
    /// File was skipped (already exists)
    Skipped,
}

/// A file to be generated
pub struct File {
    path: PathBuf,
    content: String,
    rules: FileRules,
}

impl File {
    /// Create a new file with the given path and content (default rules: always overwrite)
    pub fn new(path: impl Into<PathBuf>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content: content.into(),
            rules: FileRules::default(),
        }
    }

    /// Get the file path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the file content
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Check if the file exists
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Write the file according to its rules
    pub fn write(&self) -> Result<WriteResult> {
        match self.rules.overwrite {
            Overwrite::Always => {
                write_file(&self.path, &self.content)?;
                Ok(WriteResult::Written)
            }
            Overwrite::IfMissing => {
                if self.exists() {
                    Ok(WriteResult::Skipped)
                } else {
                    write_file(&self.path, &self.content)?;
                    Ok(WriteResult::Written)
                }
            }
        }
    }
}

/// Rules that determine how a file should be written
#[derive(Debug, Clone)]
pub struct FileRules {
    pub overwrite: Overwrite,
    pub header: Option<&'static str>,
}

/// How to handle existing files
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overwrite {
    /// Always overwrite (generated code)
    Always,
    /// Only create if file doesn't exist (stubs)
    IfMissing,
}

impl Default for FileRules {
    fn default() -> Self {
        Self {
            overwrite: Overwrite::Always,
            header: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_write_file_creates_file() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.txt");

        write_file(&path, "hello").unwrap();

        assert!(path.exists());
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello");
    }

    #[test]
    fn test_write_file_creates_parent_dirs() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("a").join("b").join("c").join("test.txt");

        write_file(&path, "nested").unwrap();

        assert!(path.exists());
        assert_eq!(fs::read_to_string(&path).unwrap(), "nested");
    }

    #[test]
    fn test_write_file_overwrites_existing() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.txt");

        write_file(&path, "first").unwrap();
        write_file(&path, "second").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "second");
    }

    #[test]
    fn test_file_write_always_overwrites() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.txt");

        fs::write(&path, "original").unwrap();

        let file = File::new(&path, "updated");
        let result = file.write().unwrap();

        assert_eq!(result, WriteResult::Written);
        assert_eq!(fs::read_to_string(&path).unwrap(), "updated");
    }

    #[test]
    fn test_file_write_if_missing_creates_new() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("new.txt");

        let file = File {
            path: path.clone(),
            content: "new content".to_string(),
            rules: FileRules {
                overwrite: Overwrite::IfMissing,
                header: None,
            },
        };
        let result = file.write().unwrap();

        assert_eq!(result, WriteResult::Written);
        assert_eq!(fs::read_to_string(&path).unwrap(), "new content");
    }

    #[test]
    fn test_file_write_if_missing_skips_existing() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("existing.txt");

        fs::write(&path, "original").unwrap();

        let file = File {
            path: path.clone(),
            content: "should not write".to_string(),
            rules: FileRules {
                overwrite: Overwrite::IfMissing,
                header: None,
            },
        };
        let result = file.write().unwrap();

        assert_eq!(result, WriteResult::Skipped);
        assert_eq!(fs::read_to_string(&path).unwrap(), "original");
    }

    #[test]
    fn test_file_exists() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.txt");

        let file = File::new(&path, "content");
        assert!(!file.exists());

        fs::write(&path, "content").unwrap();
        assert!(file.exists());
    }
}
