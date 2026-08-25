//! Bao core: the pure domain — the session model, the lifecycle state
//! machine, the alert signal, and the value types. No I/O, no `tokio`, no
//! process or filesystem access: everything here is data and rules, so it is
//! reusable by any client (native or wasm).
//!
//! This is the *domain only*. The wire contract lives in `bao-protocol`, the
//! transport in `bao-transport`, the client in `bao-client`, and all
//! OS-touching logic in `bao-daemon`.

pub mod alert;
pub mod error;
pub mod event;
pub mod lifecycle;
pub mod registry;
pub mod sandbox;
pub mod types;
