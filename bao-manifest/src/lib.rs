//! TOML manifest parsing and validation for Bao CLI generator.
//!
//! This crate handles parsing and validation of `bao.toml` configuration files
//! that define CLI structure, commands, arguments, and context.
//!
//! # Usage
//!
//! ```ignore
//! use baobao_manifest::Manifest;
//!
//! let manifest = Manifest::from_file("bao.toml")?;
//!
//! // Access CLI configuration
//! println!("CLI name: {}", manifest.cli.name);
//!
//! // Iterate over commands
//! for (name, command) in &manifest.commands {
//!     println!("Command: {}", name);
//! }
//! ```
//!
//! # Manifest Structure
//!
//! A `bao.toml` file defines:
//!
//! - **CLI metadata** - Name, version, description
//! - **Commands** - With arguments, flags, and subcommands
//! - **Context** - Shared state like database pools and HTTP clients

// Miette's derive macro generates code that triggers these warnings
#![allow(unused_assignments)]

mod command;
mod context;
mod error;
mod manifest;
mod serialize;

// Command
pub use command::{Arg, ArgType, Command, Flag};
// Context
pub use context::{
    Context, ContextField, DatabaseConfig, HttpConfig, JournalMode, MySqlConfig, PoolConfig,
    PostgresConfig, SqliteConfig, SynchronousMode,
};
// Error
pub use error::{Error, Result, SourceContext};
// Manifest
pub use manifest::{BaoToml, CliConfig, Language, Manifest, ParseContext};
