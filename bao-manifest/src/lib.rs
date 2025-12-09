// Miette's derive macro generates code that triggers these warnings
#![allow(unused_assignments)]

mod cli;
mod command;
mod context;
mod error;
mod file;
mod manifest;
mod validate;

// CLI
pub use cli::CliConfig;
// Command
pub use command::{Arg, ArgType, Command, Flag};
// Context
pub use context::{
    Context, ContextField, DatabaseConfig, HttpConfig, JournalMode, MySqlConfig, PoolConfig,
    PostgresConfig, SqliteConfig, SynchronousMode,
};
// Error
pub use error::{Error, Result, SourceContext};
// File
pub use file::BaoToml;
// Manifest
pub use manifest::Manifest;
// Validation
pub use validate::ParseContext;
