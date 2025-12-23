//! Schema traversal and info types.
//!
//! This module provides types for working with parsed schemas:
//! - [`CommandTree`] - Traversable view of command hierarchy
//! - [`FlatCommand`] - Flattened command with path information
//! - [`CommandInfo`] - Command metadata for code generation
//! - [`ContextFieldInfo`] - Context field metadata for code generation
//! - [`CommandTreeDisplay`] - Declarative command tree formatting
//! - [`ComputedData`] - Pre-computed analysis from Application IR

mod commands;
mod computed;
mod display;
mod types;

// Re-export IR types for convenience
pub use baobao_ir::{PoolConfig, SqliteOptions};
pub use commands::{CommandTree, FlatCommand};
pub use computed::ComputedData;
pub use display::{CommandTreeDisplay, CommandTreeExt, DisplayStyle};
pub use types::{
    CommandInfo, ContextFieldInfo, collect_command_paths_from_ir, collect_commands_from_ir,
    collect_context_fields, collect_context_fields_from_ir, ir_has_async,
};
