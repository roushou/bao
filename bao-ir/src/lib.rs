//! Intermediate representation types for Bao CLI generator.
//!
//! This crate provides the unified type definitions used across the Bao
//! code generation pipeline. These types serve as the single source of truth
//! for configuration and resource representation.
//!
//! # Architecture
//!
//! ```text
//! bao.toml (TOML) → bao-manifest (parsing) → bao-ir (unified types) → codegen
//! ```
//!
//! The IR types are designed to be:
//! - Language-agnostic (no Rust/TypeScript-specific concerns)
//! - Application-type agnostic (CLI, HTTP server, etc.)
//! - Serializable for debugging and visualization

mod app;
mod resource;
mod serde_helpers;
mod types;

pub use app::{
    AppIR, AppMeta, CommandOp, DatabaseResource, DefaultValue, HttpClientResource, Input,
    InputKind, InputType, Operation, Resource,
};
pub use resource::{JournalMode, PoolConfig, SqliteOptions, SynchronousMode};
pub use types::{ContextFieldInfo, ContextFieldType, DatabaseType};
