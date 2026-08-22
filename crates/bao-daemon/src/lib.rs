//! Bao daemon: the supervisor that owns sessions — the live PTY process, the
//! event log, the sandbox and harness adapters — and serves clients over the
//! wire.

mod git;

pub mod error;
pub mod harness;
pub mod hostname;
pub mod sandbox;
pub mod screen;
pub mod server;
pub mod session;

#[cfg(test)]
mod testutil;

pub use server::serve;
