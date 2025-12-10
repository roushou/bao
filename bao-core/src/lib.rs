//! Core utilities and types for Bao CLI generator.
//!
//! This crate provides fundamental types and utilities used across
//! the Bao ecosystem.

mod file;
mod type_mapper;
mod types;
mod utils;
mod version;

// File operations
pub use file::{File, FileRules, GeneratedFile, Overwrite, WriteResult};
// Fundamental types
pub use type_mapper::ArgType;
pub use types::{ContextFieldType, DatabaseType};
// String utilities
pub use utils::{
    to_camel_case, to_kebab_case, to_pascal_case, to_snake_case, toml_value_to_string,
};
pub use version::Version;
