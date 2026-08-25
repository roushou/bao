//! Bao daemon: the supervisor that owns sessions — the live PTY process, the
//! event log, the sandbox and harness adapters — and serves clients over the
//! wire.

mod git;

pub mod error;
pub mod harness;
pub mod home;
pub mod hostname;
pub mod pid;
pub mod registry;
pub mod sandbox;
pub mod screen;
pub mod server;
pub mod session;

pub use home::Home;
pub use server::serve;
