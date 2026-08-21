//! Wire payload value types: the request/response envelopes and the byte
//! encoding used on the wire.

use std::path::PathBuf;

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use serde::{Deserialize, Serialize};

use bao_core::{
    sandbox::{SandboxKind, SandboxSpec},
    types::{Command, Hostname, TerminalSize},
};

/// A byte payload carried over the JSON wire as base64.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireBytes(pub Vec<u8>);

impl Serialize for WireBytes {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&B64.encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for WireBytes {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let encoded = String::deserialize(d)?;
        B64.decode(&encoded)
            .map(WireBytes)
            .map_err(serde::de::Error::custom)
    }
}

impl From<Vec<u8>> for WireBytes {
    fn from(v: Vec<u8>) -> Self {
        Self(v)
    }
}

impl From<&[u8]> for WireBytes {
    fn from(v: &[u8]) -> Self {
        Self(v.to_vec())
    }
}

impl std::ops::Deref for WireBytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.0
    }
}

/// Wire payload for `Rpc::Launch`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchRequest {
    #[serde(default)]
    pub command: Option<Command>,
    #[serde(default)]
    pub dir: Option<PathBuf>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub size: TerminalSize,
    /// Requested isolation. `None` = resolve the strongest the machine
    /// offers for the launch dir.
    #[serde(default)]
    pub sandbox: SandboxSpec,
}

/// What the daemon says about itself — read by the client handshake and the
/// machine-facing views (sessions across machines).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonInfo {
    /// The machine this daemon runs on.
    pub host: Hostname,
    /// Wire protocol version this daemon speaks.
    pub protocol_version: u32,
    /// Isolation backends this machine can provide. A client offers only
    /// these, never more.
    pub sandbox_backends: Vec<SandboxKind>,
}
