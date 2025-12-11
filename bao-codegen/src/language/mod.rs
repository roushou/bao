//! Language-specific abstractions.
//!
//! This module provides traits and types for language-specific code generation:
//! - [`LanguageCodegen`] - Main trait for language code generators
//! - [`TypeMapper`] - Trait for mapping schema types to language types
//! - [`NamingConvention`] - Language-specific naming rules
//! - [`GenerateResult`] - Result of code generation
//! - [`CleanResult`] - Result of cleaning orphaned files
//! - [`PreviewFile`] - Generated file preview

mod naming;
mod traits;

pub use naming::NamingConvention;
pub use traits::{CleanResult, GenerateResult, LanguageCodegen, PreviewFile, TypeMapper};
