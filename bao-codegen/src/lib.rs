//! Shared code generation utilities for Bao CLI generator.
//!
//! This crate provides language-agnostic abstractions and utilities
//! used by language-specific code generators (e.g., `bao-codegen-rust`).

mod builder;
mod commands;
mod handlers;
mod imports;
pub mod mappers;
mod naming;
mod traits;
mod types;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

// Re-export traits
// Re-export utilities
pub use builder::CodeBuilder;
pub use commands::{
    CommandVisitor, FlatCommand, flatten_commands, leaf_commands, parent_commands, walk_commands,
};
pub use handlers::{HandlerPaths, collect_handler_paths, collect_leaf_handler_paths};
pub use imports::{DependencyCollector, DependencySpec, ImportCollector};
pub use naming::{GO_NAMING, NamingConvention, RUST_NAMING, TYPESCRIPT_NAMING};
pub use traits::{GenerateResult, LanguageCodegen, PreviewFile, TypeMapper};
// Re-export types
pub use types::{CommandInfo, ContextFieldInfo, PoolConfigInfo, SqliteConfigInfo};
