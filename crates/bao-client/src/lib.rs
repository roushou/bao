//! Bao client: the typed way to talk to a daemon. Frontends depend on this
//! crate (and `bao-core`); they never touch the wire vocabulary directly.

pub mod client;
pub mod error;

pub use client::{Conn, ConnWriter, HostEvent};
pub use error::Error;
