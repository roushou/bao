//! Handler file path management utilities.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use eyre::Result;

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
    /// Marker string that indicates an unmodified handler stub.
    /// Files containing this marker are considered safe to delete.
    stub_marker: String,
}

impl HandlerPaths {
    /// Create a new HandlerPaths instance.
    ///
    /// # Arguments
    /// * `base_dir` - Base directory for handlers (e.g., "src/handlers")
    /// * `extension` - File extension without dot (e.g., "rs", "ts")
    /// * `stub_marker` - Marker string that indicates an unmodified handler stub
    ///
    /// # Example
    /// ```ignore
    /// // For Rust handlers
    /// let paths = HandlerPaths::new("src/handlers", "rs", "todo!(\"implement");
    /// // For TypeScript handlers
    /// let paths = HandlerPaths::new("src/handlers", "ts", "// TODO: implement");
    /// ```
    pub fn new(
        base_dir: impl Into<PathBuf>,
        extension: impl Into<String>,
        stub_marker: impl Into<String>,
    ) -> Self {
        Self {
            base_dir: base_dir.into(),
            extension: extension.into(),
            stub_marker: stub_marker.into(),
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

    /// Find orphaned handler files with their full paths and modification status.
    ///
    /// Returns tuples of (relative_path, full_path, is_unmodified).
    /// A handler is considered unmodified if it contains the `todo!("implement` marker.
    pub fn find_orphans_with_status(
        &self,
        expected_paths: &HashSet<String>,
    ) -> Result<Vec<OrphanHandler>> {
        let mut orphans = Vec::new();
        self.scan_for_orphans_with_status(&self.base_dir, "", expected_paths, &mut orphans)?;
        Ok(orphans)
    }

    fn scan_for_orphans_with_status(
        &self,
        dir: &Path,
        prefix: &str,
        expected: &HashSet<String>,
        orphans: &mut Vec<OrphanHandler>,
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
                    // Recursively collect all files in this orphaned directory
                    self.collect_all_files(&path, &new_prefix, orphans)?;
                } else {
                    self.scan_for_orphans_with_status(&path, &new_prefix, expected, orphans)?;
                }
            } else if path
                .extension()
                .is_some_and(|ext| ext == self.extension.as_str())
            {
                let stem = path.file_stem().unwrap().to_string_lossy();
                let relative_path = if prefix.is_empty() {
                    stem.to_string()
                } else {
                    format!("{}/{}", prefix, stem)
                };

                if !expected.contains(&relative_path) {
                    let is_unmodified = self.is_handler_unmodified(&path);
                    orphans.push(OrphanHandler {
                        relative_path,
                        full_path: path,
                        is_unmodified,
                    });
                }
            }
        }

        Ok(())
    }

    /// Collect all files in an orphaned directory (recursively).
    fn collect_all_files(
        &self,
        dir: &Path,
        prefix: &str,
        orphans: &mut Vec<OrphanHandler>,
    ) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_name = path.file_name().unwrap().to_string_lossy();

            if path.is_dir() {
                let new_prefix = format!("{}/{}", prefix, file_name);
                self.collect_all_files(&path, &new_prefix, orphans)?;
            } else if path
                .extension()
                .is_some_and(|ext| ext == self.extension.as_str())
            {
                let stem = path.file_stem().unwrap().to_string_lossy();
                let relative_path = format!("{}/{}", prefix, stem);
                let is_unmodified = self.is_handler_unmodified(&path);
                orphans.push(OrphanHandler {
                    relative_path,
                    full_path: path,
                    is_unmodified,
                });
            }
        }

        Ok(())
    }

    /// Check if a handler file is unmodified (still contains the stub marker).
    fn is_handler_unmodified(&self, path: &Path) -> bool {
        std::fs::read_to_string(path)
            .map(|content| content.contains(&self.stub_marker))
            .unwrap_or(false)
    }
}

/// Information about an orphaned handler file.
#[derive(Debug, Clone)]
pub struct OrphanHandler {
    /// Path relative to the handlers directory (e.g., "db/migrate")
    pub relative_path: String,
    /// Full filesystem path
    pub full_path: PathBuf,
    /// Whether the handler is unmodified (still contains `todo!` marker)
    pub is_unmodified: bool,
}

/// Find orphaned generated command files in a directory.
///
/// Scans the given directory for files with the specified extension that are
/// not in the expected set of command names.
pub fn find_orphan_commands(
    commands_dir: &Path,
    extension: &str,
    expected_commands: &HashSet<String>,
) -> Result<Vec<PathBuf>> {
    let mut orphans = Vec::new();

    if !commands_dir.exists() {
        return Ok(orphans);
    }

    for entry in std::fs::read_dir(commands_dir)? {
        let entry = entry?;
        let path = entry.path();

        // Skip directories and mod files
        if path.is_dir() {
            continue;
        }

        let file_name = path.file_name().unwrap().to_string_lossy();
        if file_name == format!("mod.{}", extension) {
            continue;
        }

        // Check if this is a source file
        if path.extension().is_none_or(|ext| ext != extension) {
            continue;
        }

        // Get the command name (file stem)
        let stem = path.file_stem().unwrap().to_string_lossy();

        // Check if this command is expected
        if !expected_commands.contains(stem.as_ref()) {
            orphans.push(path);
        }
    }

    Ok(orphans)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUST_STUB_MARKER: &str = "todo!(\"implement";
    const TS_STUB_MARKER: &str = "// TODO: implement";

    #[test]
    fn test_handler_path_simple() {
        let paths = HandlerPaths::new("src/handlers", "rs", RUST_STUB_MARKER);
        assert_eq!(
            paths.handler_path(&["hello"]),
            PathBuf::from("src/handlers/hello.rs")
        );
    }

    #[test]
    fn test_handler_path_nested() {
        let paths = HandlerPaths::new("src/handlers", "rs", RUST_STUB_MARKER);
        assert_eq!(
            paths.handler_path(&["db", "migrate"]),
            PathBuf::from("src/handlers/db/migrate.rs")
        );
    }

    #[test]
    fn test_handler_path_deeply_nested() {
        let paths = HandlerPaths::new("src/handlers", "rs", RUST_STUB_MARKER);
        assert_eq!(
            paths.handler_path(&["config", "user", "set"]),
            PathBuf::from("src/handlers/config/user/set.rs")
        );
    }

    #[test]
    fn test_mod_path() {
        let paths = HandlerPaths::new("src/handlers", "rs", RUST_STUB_MARKER);
        assert_eq!(
            paths.mod_path(&["db"]),
            PathBuf::from("src/handlers/db/mod.rs")
        );
    }

    #[test]
    fn test_typescript_extension() {
        let paths = HandlerPaths::new("src/handlers", "ts", TS_STUB_MARKER);
        assert_eq!(
            paths.handler_path(&["hello"]),
            PathBuf::from("src/handlers/hello.ts")
        );
    }
}
