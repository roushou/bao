//! Transport addressing: where the daemon listens or a client connects.

use std::{
    fmt,
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
    str::FromStr,
};

use serde::{Deserialize, Serialize};

use crate::error::Error;

pub const DEFAULT_PORT: u16 = 14551;

/// Where the daemon listens or a client connects: a TCP host:port, or a
/// unix socket path for local-only transport. The transport is part of the
/// address, so dialing and binding are both driven by one value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Addr {
    /// TCP host:port (loopback by default; explicit `--port` for remote).
    Tcp { host: IpAddr, port: u16 },
    /// A unix socket path — local-only, trust via filesystem permissions.
    Unix(PathBuf),
}

impl Addr {
    pub fn local(port: u16) -> Self {
        Self::Tcp {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port,
        }
    }

    pub fn localhost() -> Self {
        Self::local(DEFAULT_PORT)
    }

    /// A unix-socket address for local-only transport.
    pub fn unix(path: impl Into<PathBuf>) -> Self {
        Self::Unix(path.into())
    }
}

impl Default for Addr {
    fn default() -> Self {
        Self::localhost()
    }
}

impl fmt::Display for Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Addr::Tcp { host, port } => write!(f, "{host}:{port}"),
            Addr::Unix(path) => write!(f, "unix:{}", path.display()),
        }
    }
}

impl FromStr for Addr {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        if let Some(path) = s.strip_prefix("unix:") {
            if path.is_empty() {
                return Err(Error::BadAddr);
            }
            return Ok(Addr::Unix(PathBuf::from(path)));
        }
        let (host, port) = s.rsplit_once(':').ok_or(Error::BadAddr)?;
        let host = host.parse().map_err(|_| Error::BadAddr)?;
        let port = port.parse().map_err(|_| Error::BadAddr)?;
        Ok(Addr::Tcp { host, port })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tcp_parses_and_displays() {
        let a: Addr = "127.0.0.1:14551".parse().unwrap();
        assert_eq!(a, Addr::local(14551));
        assert_eq!(a.to_string(), "127.0.0.1:14551");
        assert!(matches!(
            Addr::localhost(),
            Addr::Tcp {
                port: DEFAULT_PORT,
                ..
            }
        ));
    }

    #[test]
    fn unix_parses_and_displays() {
        let a: Addr = "unix:/tmp/bao.sock".parse().unwrap();
        assert_eq!(a, Addr::Unix("/tmp/bao.sock".into()));
        assert_eq!(a.to_string(), "unix:/tmp/bao.sock");
        assert!(matches!(Addr::unix("/s"), Addr::Unix(_)));
    }

    #[test]
    fn rejects_garbage() {
        assert!("".parse::<Addr>().is_err());
        assert!("unix:".parse::<Addr>().is_err());
        assert!("nonsense".parse::<Addr>().is_err());
        assert!("host:notaport".parse::<Addr>().is_err());
    }
}
