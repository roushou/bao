//! Client crate error type — branchable by consumers, no `anyhow`.

use bao_core::{protocol::WireError, types::Addr};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Core(#[from] bao_core::error::Error),
    #[error("cannot reach bao host at {addr} — is `bao daemon` running? ({source})")]
    Unreachable { addr: Addr, source: std::io::Error },
    #[error("host error: {0}")]
    Rpc(WireError),
    #[error("protocol version mismatch: server speaks v{server}, this client speaks v{client}")]
    VersionMismatch { server: u32, client: u32 },
    #[error("lost connection to host")]
    LostConnection,
    #[error("unexpected reply from host")]
    UnexpectedReply,
}
