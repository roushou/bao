//! Language-agnostic code generation traits.

use std::path::Path;

use eyre::Result;

/// Trait for language-specific code generators.
///
/// Implement this trait to add support for generating CLI code in a new language.
pub trait LanguageCodegen {
    /// Language identifier (e.g., "rust", "typescript", "python")
    fn language(&self) -> &'static str;

    /// File extension for generated source files (e.g., "rs", "ts", "py")
    fn file_extension(&self) -> &'static str;

    /// Preview generated files without writing to disk
    fn preview(&self) -> Vec<PreviewFile>;

    /// Generate all files into the specified output directory
    fn generate(&self, output_dir: &Path) -> Result<GenerateResult>;
}

/// Result of code generation
#[derive(Debug, Default)]
pub struct GenerateResult {
    /// Handler files that were created (stubs for new commands)
    pub created_handlers: Vec<String>,
    /// Handler files that exist but are no longer used
    pub orphan_handlers: Vec<String>,
}

/// A generated file for preview
#[derive(Debug)]
pub struct PreviewFile {
    /// Relative path from output directory
    pub path: String,
    /// File content
    pub content: String,
}
