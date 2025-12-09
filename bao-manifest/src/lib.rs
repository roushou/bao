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
pub use manifest::{BaoToml, CliConfig, Manifest, ParseContext};
