//! File registration pattern for declarative code generation.
//!
//! This module provides a registry-based approach to file generation that:
//! - Makes file generation declarative
//! - Centralizes file metadata (path, category, overwrite rules)
//! - Enables dependency tracking between files
//! - Simplifies preview and generation logic
//!
//! # Example
//!
//! ```ignore
//! let mut registry = FileRegistry::new();
//!
//! // Register infrastructure files (always overwritten)
//! registry.register(FileEntry::always("Cargo.toml", cargo_toml.render()));
//! registry.register(FileEntry::always("src/main.rs", main_rs.render()));
//!
//! // Register handler stubs (only if missing)
//! registry.register(FileEntry::if_missing("src/handlers/hello.rs", stub.render()));
//!
//! // Generate all files
//! registry.write_all(&output_dir)?;
//! ```

use std::path::{Path, PathBuf};

use baobao_core::{FileRules, GeneratedFile, Overwrite, WriteResult};
use eyre::Result;

/// Category of generated file, determining generation order and behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileCategory {
    /// Project configuration files (Cargo.toml, package.json, etc.)
    /// Generated first, always overwritten.
    Config,
    /// Core infrastructure files (main.rs, index.ts, etc.)
    /// Generated second, always overwritten.
    Infrastructure,
    /// Generated code files (cli.rs, context.rs, commands/*.rs)
    /// Generated third, always overwritten.
    Generated,
    /// Handler stub files that users edit.
    /// Generated last, only if missing.
    Handler,
}

impl FileCategory {
    /// Get the default overwrite behavior for this category.
    pub fn default_overwrite(&self) -> Overwrite {
        match self {
            FileCategory::Handler => Overwrite::IfMissing,
            _ => Overwrite::Always,
        }
    }
}

/// An entry in the file registry representing a file to be generated.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Relative path from output directory.
    pub path: String,
    /// File content.
    pub content: String,
    /// Category determining generation behavior.
    pub category: FileCategory,
    /// Override default overwrite behavior.
    pub overwrite: Option<Overwrite>,
}

impl FileEntry {
    /// Create a new file entry with the given category.
    pub fn new(
        path: impl Into<String>,
        content: impl Into<String>,
        category: FileCategory,
    ) -> Self {
        Self {
            path: path.into(),
            content: content.into(),
            category,
            overwrite: None,
        }
    }

    /// Create a config file (always overwritten, generated first).
    pub fn config(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self::new(path, content, FileCategory::Config)
    }

    /// Create an infrastructure file (always overwritten).
    pub fn infrastructure(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self::new(path, content, FileCategory::Infrastructure)
    }

    /// Create a generated code file (always overwritten).
    pub fn generated(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self::new(path, content, FileCategory::Generated)
    }

    /// Create a handler stub file (only if missing).
    pub fn handler(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self::new(path, content, FileCategory::Handler)
    }

    /// Create from a GeneratedFile, respecting its rules.
    pub fn from_generated<F: GeneratedFile>(
        path: impl Into<String>,
        file: &F,
        category: FileCategory,
    ) -> Self {
        Self::new(path, file.render(), category).with_overwrite(file.rules().overwrite)
    }

    /// Override the default overwrite behavior.
    pub fn with_overwrite(mut self, overwrite: Overwrite) -> Self {
        self.overwrite = Some(overwrite);
        self
    }

    /// Get the effective overwrite behavior.
    pub fn overwrite(&self) -> Overwrite {
        self.overwrite
            .unwrap_or_else(|| self.category.default_overwrite())
    }

    /// Get the file rules for this entry.
    pub fn rules(&self) -> FileRules {
        FileRules {
            overwrite: self.overwrite(),
            header: None,
        }
    }

    /// Get the full path for this entry.
    pub fn full_path(&self, base: &Path) -> PathBuf {
        base.join(&self.path)
    }

    /// Write this file to disk.
    pub fn write(&self, base: &Path) -> Result<WriteResult> {
        let path = self.full_path(base);
        let overwrite = self.overwrite();

        match overwrite {
            Overwrite::Always => {
                write_file(&path, &self.content)?;
                Ok(WriteResult::Written)
            }
            Overwrite::IfMissing => {
                if path.exists() {
                    Ok(WriteResult::Skipped)
                } else {
                    write_file(&path, &self.content)?;
                    Ok(WriteResult::Written)
                }
            }
        }
    }
}

/// Registry for collecting and managing generated files.
///
/// Files are stored by category and generated in category order:
/// Config -> Infrastructure -> Generated -> Handler
#[derive(Debug, Default)]
pub struct FileRegistry {
    entries: Vec<FileEntry>,
}

impl FileRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a file entry.
    pub fn register(&mut self, entry: FileEntry) {
        self.entries.push(entry);
    }

    /// Register multiple file entries.
    pub fn register_all(&mut self, entries: impl IntoIterator<Item = FileEntry>) {
        self.entries.extend(entries);
    }

    /// Get all registered entries, sorted by category.
    pub fn entries(&self) -> impl Iterator<Item = &FileEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by_key(|e| e.category);
        sorted.into_iter()
    }

    /// Get entries for a specific category.
    pub fn entries_by_category(&self, category: FileCategory) -> impl Iterator<Item = &FileEntry> {
        self.entries.iter().filter(move |e| e.category == category)
    }

    /// Get the number of registered entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Preview all files (returns path and content pairs).
    pub fn preview(&self) -> Vec<PreviewEntry> {
        self.entries()
            .map(|e| PreviewEntry {
                path: e.path.clone(),
                content: e.content.clone(),
                category: e.category,
            })
            .collect()
    }

    /// Write all files to the output directory.
    ///
    /// Files are written in category order. Returns statistics about what was written.
    pub fn write_all(&self, base: &Path) -> Result<WriteStats> {
        let mut stats = WriteStats::default();

        for entry in self.entries() {
            match entry.write(base)? {
                WriteResult::Written => {
                    stats.written += 1;
                    stats.written_paths.push(entry.path.clone());
                }
                WriteResult::Skipped => {
                    stats.skipped += 1;
                    stats.skipped_paths.push(entry.path.clone());
                }
            }
        }

        Ok(stats)
    }

    /// Clear all registered entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// A preview entry for displaying what would be generated.
#[derive(Debug, Clone)]
pub struct PreviewEntry {
    /// Relative path from output directory.
    pub path: String,
    /// File content.
    pub content: String,
    /// File category.
    pub category: FileCategory,
}

/// Statistics from a write operation.
#[derive(Debug, Default)]
pub struct WriteStats {
    /// Number of files written.
    pub written: usize,
    /// Number of files skipped (already existed).
    pub skipped: usize,
    /// Paths of written files.
    pub written_paths: Vec<String>,
    /// Paths of skipped files.
    pub skipped_paths: Vec<String>,
}

impl WriteStats {
    /// Total number of files processed.
    pub fn total(&self) -> usize {
        self.written + self.skipped
    }
}

fn write_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_file_entry_categories() {
        let config = FileEntry::config("Cargo.toml", "");
        assert_eq!(config.category, FileCategory::Config);
        assert_eq!(config.overwrite(), Overwrite::Always);

        let handler = FileEntry::handler("src/handlers/hello.rs", "");
        assert_eq!(handler.category, FileCategory::Handler);
        assert_eq!(handler.overwrite(), Overwrite::IfMissing);
    }

    #[test]
    fn test_registry_ordering() {
        let mut registry = FileRegistry::new();

        // Register in random order
        registry.register(FileEntry::handler("handler.rs", ""));
        registry.register(FileEntry::config("config.toml", ""));
        registry.register(FileEntry::generated("generated.rs", ""));
        registry.register(FileEntry::infrastructure("main.rs", ""));

        // Should come out in category order
        let paths: Vec<_> = registry.entries().map(|e| e.path.as_str()).collect();
        assert_eq!(
            paths,
            vec!["config.toml", "main.rs", "generated.rs", "handler.rs"]
        );
    }

    #[test]
    fn test_registry_write_all() {
        let temp = TempDir::new().unwrap();
        let mut registry = FileRegistry::new();

        registry.register(FileEntry::config("test.txt", "content"));
        registry.register(FileEntry::handler("stub.txt", "stub"));

        let stats = registry.write_all(temp.path()).unwrap();

        assert_eq!(stats.written, 2);
        assert_eq!(stats.skipped, 0);
        assert!(temp.path().join("test.txt").exists());
        assert!(temp.path().join("stub.txt").exists());
    }

    #[test]
    fn test_handler_skipped_if_exists() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("handler.rs");

        // Create existing file
        std::fs::write(&path, "user code").unwrap();

        let mut registry = FileRegistry::new();
        registry.register(FileEntry::handler("handler.rs", "stub"));

        let stats = registry.write_all(temp.path()).unwrap();

        assert_eq!(stats.written, 0);
        assert_eq!(stats.skipped, 1);
        // Original content preserved
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "user code");
    }

    #[test]
    fn test_preview() {
        let mut registry = FileRegistry::new();
        registry.register(FileEntry::config("a.txt", "content a"));
        registry.register(FileEntry::generated("b.txt", "content b"));

        let preview = registry.preview();

        assert_eq!(preview.len(), 2);
        assert_eq!(preview[0].path, "a.txt");
        assert_eq!(preview[1].path, "b.txt");
    }
}
