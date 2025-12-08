//! Handler file path management utilities.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use baobao_schema::Schema;
use eyre::Result;

use crate::commands::flatten_commands;

/// Manages handler file paths for a code generator.
///
/// Provides utilities for:
/// - Computing handler file paths from command paths
/// - Finding orphaned handler files
/// - Checking if handlers exist
#[derive(Debug, Clone)]
pub struct HandlerPaths {
    /// Base directory for handlers (e.g., "src/handlers")
    base_dir: PathBuf,
    /// File extension without dot (e.g., "rs", "ts")
    extension: String,
}

impl HandlerPaths {
    /// Create a new HandlerPaths instance.
    pub fn new(base_dir: impl Into<PathBuf>, extension: impl Into<String>) -> Self {
        Self {
            base_dir: base_dir.into(),
            extension: extension.into(),
        }
    }

    /// Get the handler file path for a command.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let paths = HandlerPaths::new("src/handlers", "rs");
    /// assert_eq!(
    ///     paths.handler_path(&["db", "migrate"]),
    ///     PathBuf::from("src/handlers/db/migrate.rs")
    /// );
    /// ```
    pub fn handler_path(&self, command_path: &[&str]) -> PathBuf {
        let mut path = self.base_dir.clone();
        if command_path.len() > 1 {
            // Add parent directories
            for segment in &command_path[..command_path.len() - 1] {
                path.push(segment);
            }
        }
        // Add the file with extension
        if let Some(last) = command_path.last() {
            path.push(format!("{}.{}", last, self.extension));
        }
        path
    }

    /// Get the module file path for a parent command.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let paths = HandlerPaths::new("src/handlers", "rs");
    /// assert_eq!(
    ///     paths.mod_path(&["db"]),
    ///     PathBuf::from("src/handlers/db/mod.rs")
    /// );
    /// ```
    pub fn mod_path(&self, command_path: &[&str]) -> PathBuf {
        let mut path = self.base_dir.clone();
        for segment in command_path {
            path.push(segment);
        }
        path.push(format!("mod.{}", self.extension));
        path
    }

    /// Check if a handler file exists.
    pub fn exists(&self, command_path: &[&str]) -> bool {
        self.handler_path(command_path).exists()
    }

    /// Find orphaned handler files that are no longer in the schema.
    ///
    /// Returns paths relative to the base directory.
    pub fn find_orphans(&self, expected_paths: &HashSet<String>) -> Result<Vec<String>> {
        let mut orphans = Vec::new();
        self.scan_for_orphans(&self.base_dir, "", expected_paths, &mut orphans)?;
        Ok(orphans)
    }

    fn scan_for_orphans(
        &self,
        dir: &Path,
        prefix: &str,
        expected: &HashSet<String>,
        orphans: &mut Vec<String>,
    ) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_name = path.file_name().unwrap().to_string_lossy();

            // Skip mod files
            if file_name == format!("mod.{}", self.extension) {
                continue;
            }

            if path.is_dir() {
                let new_prefix = if prefix.is_empty() {
                    file_name.to_string()
                } else {
                    format!("{}/{}", prefix, file_name)
                };

                // Check if this directory is expected
                if !expected.contains(&new_prefix) {
                    orphans.push(new_prefix.clone());
                } else {
                    self.scan_for_orphans(&path, &new_prefix, expected, orphans)?;
                }
            } else if path
                .extension()
                .is_some_and(|ext| ext == self.extension.as_str())
            {
                let stem = path.file_stem().unwrap().to_string_lossy();
                let handler_path = if prefix.is_empty() {
                    stem.to_string()
                } else {
                    format!("{}/{}", prefix, stem)
                };

                if !expected.contains(&handler_path) {
                    orphans.push(handler_path);
                }
            }
        }

        Ok(())
    }
}

/// Collect all handler paths from a schema.
///
/// Returns a set of path strings like "db/migrate", "hello".
pub fn collect_handler_paths(schema: &Schema) -> HashSet<String> {
    let commands = flatten_commands(schema);
    let mut paths = HashSet::new();

    for cmd in commands {
        let path_str = cmd.path_str("/");
        paths.insert(path_str);
    }

    paths
}

/// Collect only leaf handler paths (actual handler files, not parent directories).
pub fn collect_leaf_handler_paths(schema: &Schema) -> HashSet<String> {
    let commands = flatten_commands(schema);
    let mut paths = HashSet::new();

    for cmd in commands {
        if cmd.is_leaf {
            paths.insert(cmd.path_str("/"));
        }
    }

    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_path_simple() {
        let paths = HandlerPaths::new("src/handlers", "rs");
        assert_eq!(
            paths.handler_path(&["hello"]),
            PathBuf::from("src/handlers/hello.rs")
        );
    }

    #[test]
    fn test_handler_path_nested() {
        let paths = HandlerPaths::new("src/handlers", "rs");
        assert_eq!(
            paths.handler_path(&["db", "migrate"]),
            PathBuf::from("src/handlers/db/migrate.rs")
        );
    }

    #[test]
    fn test_handler_path_deeply_nested() {
        let paths = HandlerPaths::new("src/handlers", "rs");
        assert_eq!(
            paths.handler_path(&["config", "user", "set"]),
            PathBuf::from("src/handlers/config/user/set.rs")
        );
    }

    #[test]
    fn test_mod_path() {
        let paths = HandlerPaths::new("src/handlers", "rs");
        assert_eq!(
            paths.mod_path(&["db"]),
            PathBuf::from("src/handlers/db/mod.rs")
        );
    }

    #[test]
    fn test_typescript_extension() {
        let paths = HandlerPaths::new("src/handlers", "ts");
        assert_eq!(
            paths.handler_path(&["hello"]),
            PathBuf::from("src/handlers/hello.ts")
        );
    }
}
