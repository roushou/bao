//! Bao protocol: the wire contract — the typed message vocabulary that the
//! client and the daemon speak. Pure data: `serde` only, no tokio, no I/O.
//!
//! This is the *contract*, not the transport. Framing and the socket layer
//! live in `bao-transport`; the typed client lives in `bao-client`; the
//! server half lives in `bao-daemon`.

mod protocol;
mod types;

pub use protocol::{ChannelKind, FromHost, PROTOCOL_VERSION, Reply, Request, Rpc, WireError};
pub use types::{DaemonInfo, LaunchRequest, WireBytes};
