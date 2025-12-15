//! Language-agnostic code generation traits.

use std::path::Path;

use baobao_core::{ArgType, ContextFieldType};
use eyre::Result;

/// Trait for language-specific code generators.
///
/// Implement this trait to add support for generating CLI code in a new language.
pub trait LanguageCodegen {
    /// Language identifier (e.g., "rust", "typescript", "go")
    fn language(&self) -> &'static str;

    /// File extension for generated source files (e.g., "rs", "ts", "go")
    fn file_extension(&self) -> &'static str;

    /// Preview generated files without writing to disk
    fn preview(&self) -> Vec<PreviewFile>;

    /// Generate all files into the specified output directory
    fn generate(&self, output_dir: &Path) -> Result<GenerateResult>;

    /// Clean orphaned generated files.
    ///
    /// Removes:
    /// - Generated command files that are no longer in the manifest
    /// - Unmodified handler stubs that are no longer in the manifest
    ///
    /// Handler files that have been modified by the user are not deleted.
    ///
    /// Default implementation returns an empty result (no cleaning).
    fn clean(&self, _output_dir: &Path) -> Result<CleanResult> {
        Ok(CleanResult::default())
    }

    /// Preview what would be cleaned without actually deleting files.
    ///
    /// Default implementation returns an empty result.
    fn preview_clean(&self, _output_dir: &Path) -> Result<CleanResult> {
        Ok(CleanResult::default())
    }
}

/// Result of code generation
#[derive(Debug, Default)]
pub struct GenerateResult {
    /// Handler files that were created (stubs for new commands)
    pub created_handlers: Vec<String>,
    /// Handler files that exist but are no longer used
    pub orphan_handlers: Vec<String>,
}

/// Result of cleaning orphaned files
#[derive(Debug, Default)]
pub struct CleanResult {
    /// Generated command files that were deleted
    pub deleted_commands: Vec<String>,
    /// Handler files that were deleted (unmodified stubs)
    pub deleted_handlers: Vec<String>,
    /// Handler files that were skipped (modified by user)
    pub skipped_handlers: Vec<String>,
}

/// A generated file for preview
#[derive(Debug)]
pub struct PreviewFile {
    /// Relative path from output directory
    pub path: String,
    /// File content
    pub content: String,
}

/// Trait for mapping schema types to language-specific type strings.
///
/// Implement this trait for each target language to provide type mappings.
pub trait TypeMapper {
    /// The target language name
    fn language(&self) -> &'static str;

    /// Map an argument type to a language-specific type string
    fn map_arg_type(&self, arg_type: ArgType) -> &'static str;

    /// Map an optional argument type (e.g., `Option<String>` in Rust, `string | undefined` in TS)
    fn map_optional_arg_type(&self, arg_type: ArgType) -> String {
        // Default implementation - languages can override
        format!("Option<{}>", self.map_arg_type(arg_type))
    }

    /// Map a context field type to a language-specific type string
    fn map_context_type(&self, field_type: &ContextFieldType) -> &'static str;
}
