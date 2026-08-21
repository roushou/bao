//! Client crate error type — branchable by consumers, no `anyhow`.

use bao_protocol::WireError;
use bao_transport::Addr;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("{0}")]
    Transport(#[from] bao_transport::Error),
    #[error("the {0} transport is not wired up yet")]
    TransportUnsupported(&'static str),
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
