//! Schema traversal and info types.
//!
//! This module provides types for working with parsed schemas:
//! - [`CommandTree`] - Traversable view of command hierarchy
//! - [`FlatCommand`] - Flattened command with path information
//! - [`CommandInfo`] - Command metadata for code generation
//! - [`ContextFieldInfo`] - Context field metadata for code generation

mod commands;
mod types;

pub use commands::{CommandTree, FlatCommand};
pub use types::{
    CommandInfo, ContextFieldInfo, PoolConfigInfo, SqliteConfigInfo, collect_context_fields,
};
