//! Adapter abstractions for framework-specific code generation.
//!
//! This module provides traits that abstract away framework-specific details,
//! allowing generators to be decoupled from specific libraries like clap, sqlx, etc.
//!
//! # Architecture
//!
//! Adapters are compile-time abstractions using generics for zero overhead.
//! Each language generator is parameterized by adapter implementations.
//!
//! # Available Adapters
//!
//! - [`CliAdapter`] - CLI framework abstraction (clap, argh, boune, etc.)
//! - [`DatabaseAdapter`] - Database connection/pool abstraction (sqlx, diesel, etc.)
//! - [`RuntimeAdapter`] - Async runtime abstraction (tokio, async-std, etc.)
//! - [`ErrorAdapter`] - Error handling abstraction (eyre, anyhow, etc.)

mod async_runtime;
mod cli;
mod database;
mod error;

pub use async_runtime::{RuntimeAdapter, RuntimeInfo};
pub use cli::{
    ArgMeta, CliAdapter, CliInfo, CommandMeta, Dependency, DispatchInfo, FlagMeta, ImportSpec,
    SubcommandMeta,
};
pub use database::{DatabaseAdapter, DatabaseOptionsInfo, PoolConfig, PoolInitInfo, SqliteConfig};
pub use error::ErrorAdapter;
