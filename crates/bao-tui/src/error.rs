//! TUI crate error type — branchable by consumers, no `anyhow`.

use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("a terminal is required (stdin/stdout are not TTYs)")]
    NotATerminal,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Client(#[from] bao_wire::error::Error),
}
