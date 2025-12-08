// Miette's derive macro generates code that triggers these warnings
#![allow(unused_assignments)]

mod cli;
mod command;
mod context;
mod error;
mod file;
mod manifest;
pub(crate) mod validate;

// CLI
pub use cli::CliConfig;
// Command
pub use command::{Arg, ArgType, Command, Flag};
// Context
pub use context::{
    Context, ContextField, HttpConfig, JournalMode, MySqlConfig, PoolConfig, PostgresConfig,
    SqliteConfig, SynchronousMode,
};
// Error
pub use error::{Error, Result};
// File
pub use file::BaoToml;
// Manifest
pub use manifest::Manifest;
