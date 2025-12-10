//! Shared code generation utilities for Bao CLI generator.
//!
//! This crate provides language-agnostic abstractions and utilities
//! used by language-specific code generators (e.g., `bao-codegen-rust`).

mod bao_toml;
mod code_builder;
mod commands;
mod file_builder;
mod handlers;
mod imports;
mod indent;
mod naming;
mod traits;
mod types;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

// Re-export utilities
pub use bao_toml::BaoToml;
pub use code_builder::CodeBuilder;
pub use commands::{CommandTree, FlatCommand};
pub use file_builder::FileBuilder;
pub use handlers::{HandlerPaths, OrphanHandler, find_orphan_commands};
pub use imports::{DependencyCollector, DependencySpec, ImportCollector};
pub use indent::Indent;
pub use naming::NamingConvention;
pub use traits::{CleanResult, GenerateResult, LanguageCodegen, PreviewFile, TypeMapper};
// Re-export types
pub use types::{CommandInfo, ContextFieldInfo, PoolConfigInfo, SqliteConfigInfo};
