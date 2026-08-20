//! Bao core: the pure domain — the session model, the lifecycle state
//! machine, the alert signal, the wire protocol types, and the value
//! types. No I/O, no `tokio`, no process or filesystem access: everything
//! here is data and rules, so it is reusable by any client (native or wasm).

pub mod alert;
pub mod error;
pub mod event;
pub mod lifecycle;
pub mod protocol;
pub mod sandbox;
pub mod types;
