//! Schema traversal and info types.
//!
//! This module provides types for working with parsed schemas:
//! - [`CommandTree`] - Traversable view of command hierarchy with display methods
//! - [`FlatCommand`] - Flattened command with path information
//! - [`CommandTreeDisplay`] - Declarative command tree formatting
//! - [`DisplayStyle`] - Display style options for command trees
//! - [`ComputedData`] - Pre-computed analysis from Application IR

mod commands;
mod computed;
mod display;

// Re-export IR types for convenience
pub use baobao_ir::{ContextFieldInfo, PoolConfig, SqliteOptions};
pub use commands::{CommandTree, FlatCommand};
pub use computed::ComputedData;
pub use display::{CommandTreeDisplay, DisplayStyle};
