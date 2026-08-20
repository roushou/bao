//! Errors from a harness adapter.

use thiserror::Error as ThisError;

/// Errors from a harness adapter. `Unsupported` is the honest answer of a
/// capability this harness cannot provide (e.g. pack for the fallback).
#[derive(Debug, ThisError)]
pub enum Error {
    #[error("this harness cannot {0}")]
    Unsupported(&'static str),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
