//! The wire message vocabulary: `Request`/`Rpc` client→daemon, `FromHost`
//! daemon→client, with `Reply` payloads. The JSON shape is an implementation
//! detail of serde; nothing is matched as a string on the Rust side.

use serde::{Deserialize, Serialize};

use bao_core::{
    event::{EventKind, SessionEvent},
    sandbox::SandboxKind,
    types::{SessionId, SessionMeta, Status, TerminalSize},
};

use crate::types::{DaemonInfo, LaunchRequest, WireBytes};

/// The wire protocol version. Bump on breaking wire changes; a client whose
/// version differs from the daemon's refuses to talk rather than misparse.
pub const PROTOCOL_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Channels
// ---------------------------------------------------------------------------

/// What a connection carries. The **first frame on every connection** names
/// its channel, so the daemon dispatches per stream and a channel's lifetime
/// is its connection's — cancellation is the socket closing, and each
/// channel gets its own backpressure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChannelKind {
    /// Typed RPC request/reply.
    Control,
    /// Daemon-wide derived state (current + changes).
    Watch,
    /// One session's terminal bytes, live.
    Attach { session: SessionId },
}

// ---------------------------------------------------------------------------
// Client -> daemon
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: u32,
    #[serde(flatten)]
    pub rpc: Rpc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "m", rename_all = "snake_case")]
pub enum Rpc {
    /// The channel handshake: sent as the first frame on every connection to
    /// name the channel it carries.
    Hello {
        kind: ChannelKind,
    },
    List,
    /// The daemon's self-description (host, protocol version, capabilities).
    /// Sent right after the Hello by every control channel.
    Info,
    Launch(LaunchRequest),
    Input {
        session: SessionId,
        data: WireBytes,
    },
    Resume {
        session: SessionId,
        size: TerminalSize,
    },
    Resize {
        session: SessionId,
        size: TerminalSize,
    },
    Stop {
        session: SessionId,
    },
    Rename {
        session: SessionId,
        /// `None` clears the name.
        name: Option<String>,
    },
    Rm {
        session: SessionId,
    },
}

// ---------------------------------------------------------------------------
// Daemon -> client
// ---------------------------------------------------------------------------

/// Success payload of an RPC, typed per method.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "r", rename_all = "snake_case")]
pub enum Reply {
    List {
        sessions: Vec<SessionMeta>,
    },
    Info {
        info: DaemonInfo,
    },
    Launch {
        session: SessionMeta,
    },
    Resume {
        session: SessionMeta,
    },
    Attach {
        session: SessionMeta,
        /// The event-log sequence the live stream follows from.
        seq: u64,
        /// The current screen, rebuilt as a byte stream — feed it to a
        /// fresh emulator to render the terminal's state without replay.
        screen: WireBytes,
    },
    Ok,
}

/// Everything the daemon pushes on a connection: RPC replies, errors, and the
/// live session stream.
/// Transient message payloads are encoded to bytes immediately, so the enum
/// is moved, never copied — the size spread is intentional.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum FromHost {
    Reply {
        id: u32,
        #[serde(flatten)]
        reply: Reply,
    },
    Err {
        id: u32,
        error: WireError,
    },
    Output {
        session: SessionId,
        seq: u64,
        data: WireBytes,
        ts: u64,
    },
    /// A session's complete current picture, pushed on any change (status,
    /// output, idle). Stateless views replace their model with this blob;
    /// nothing here needs deriving client-side.
    State {
        ts: u64,
        meta: SessionMeta,
    },
    Status {
        session: SessionId,
        seq: u64,
        status: Status,
        ts: u64,
    },
    /// A session was removed — a rolled-back launch, or an `rm`. Watchers
    /// drop it from the overview. `reason` is `Some` when a launch failed
    /// and was compensated; `None` for an ordinary removal.
    Gone {
        session: SessionId,
        reason: Option<String>,
    },
}

/// A typed error the daemon sends instead of a bare string — clients can
/// branch on the kind (not-found vs already-running vs isolation).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum WireError {
    NotFound {
        query: String,
    },
    Ambiguous {
        query: String,
        ids: usize,
        names: usize,
    },
    AlreadyRunning,
    NotRunning,
    SandboxUnavailable {
        kind: SandboxKind,
    },
    /// The request was wrong, not the daemon (bad args, bad command).
    BadRequest {
        message: String,
    },
    /// The daemon failed unexpectedly; the message is the honest detail.
    Internal {
        message: String,
    },
}

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WireError::NotFound { query } => write!(f, "no session matches '{query}'"),
            WireError::Ambiguous { query, ids, names } => write!(
                f,
                "'{query}' is ambiguous ({ids} id(s), {names} name(s)) — be more specific"
            ),
            WireError::AlreadyRunning => write!(f, "session already has a running process"),
            WireError::NotRunning => write!(f, "session is not running (interrupted)"),
            WireError::SandboxUnavailable { kind } => write!(
                f,
                "sandbox {kind} is not available here (not a git repo, or the backend is missing)"
            ),
            WireError::BadRequest { message } => write!(f, "{message}"),
            WireError::Internal { message } => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for WireError {}

impl From<&str> for WireError {
    fn from(s: &str) -> Self {
        WireError::BadRequest {
            message: s.to_string(),
        }
    }
}

impl From<String> for WireError {
    fn from(s: String) -> Self {
        WireError::BadRequest { message: s }
    }
}

impl From<std::io::Error> for WireError {
    fn from(e: std::io::Error) -> Self {
        WireError::BadRequest {
            message: e.to_string(),
        }
    }
}

/// Domain validation errors are, by construction, a bad request — the daemon
/// is fine; the client asked for something invalid.
impl From<&bao_core::error::Error> for WireError {
    fn from(e: &bao_core::error::Error) -> Self {
        WireError::BadRequest {
            message: e.to_string(),
        }
    }
}

impl From<bao_core::error::Error> for WireError {
    fn from(e: bao_core::error::Error) -> Self {
        WireError::from(&e)
    }
}

impl From<&SessionEvent> for FromHost {
    fn from(ev: &SessionEvent) -> Self {
        match &ev.kind {
            EventKind::Output(bytes) => FromHost::Output {
                session: ev.session.clone(),
                seq: ev.seq,
                data: WireBytes(bytes.clone()),
                ts: ev.ts,
            },
            EventKind::Status(st) => FromHost::Status {
                session: ev.session.clone(),
                seq: ev.seq,
                status: *st,
                ts: ev.ts,
            },
        }
    }
}
