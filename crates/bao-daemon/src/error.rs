//! The daemon's error type: everything the supervisor can fail at — I/O,
//! PTY, git/sandbox, spawn, and the domain rules it enforces (wrapped).
//! Converted to a typed [`bao_protocol::WireError`] at the server boundary.

use bao_core::{sandbox::SandboxKind, types::Status};
use bao_protocol::WireError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("{0}")]
    Domain(#[from] bao_core::error::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("base64 error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("{0}")]
    Transport(#[from] bao_transport::Error),
    #[error("pty error: {0}")]
    Pty(String),
    #[error("the {0} transport is not wired up yet")]
    TransportUnsupported(&'static str),
    #[error("no session matches '{0}'")]
    NotFound(String),
    #[error("'{0}' is ambiguous ({1} id(s), {2} name(s)) — be more specific")]
    Ambiguous(String, usize, usize),
    #[error("session is not running (interrupted)")]
    NotRunning,
    #[error("session already has a running process")]
    AlreadyRunning,
    #[error("only interrupted sessions can be resumed (this one is {0})")]
    ResumeNotInterrupted(Status),
    #[error("not a git repository")]
    NotAGitRepo,
    #[error("git error: {0}")]
    Git(String),
    #[error("sandbox {0} is not available here (not a git repo, or the backend is missing)")]
    SandboxUnavailable(SandboxKind),
    #[error("worktree error: {0}")]
    Worktree(String),
    #[error("failed to spawn: {0}")]
    Spawn(String),
    #[error("session meta uses format {0}, newer than this Bao supports")]
    NewerFormat(u32),
}

impl From<&Error> for WireError {
    fn from(e: &Error) -> WireError {
        match e {
            Error::Domain(d) => WireError::from(d),
            Error::NotFound(q) => WireError::NotFound { query: q.clone() },
            Error::Ambiguous(q, ids, names) => WireError::Ambiguous {
                query: q.clone(),
                ids: *ids,
                names: *names,
            },
            Error::AlreadyRunning => WireError::AlreadyRunning,
            Error::NotRunning => WireError::NotRunning,
            Error::SandboxUnavailable(kind) => WireError::SandboxUnavailable { kind: *kind },
            // User-correctable: they asked for something the daemon can't or
            // won't do given the current state.
            Error::ResumeNotInterrupted(_)
            | Error::NotAGitRepo
            | Error::TransportUnsupported(_) => WireError::BadRequest {
                message: e.to_string(),
            },
            // The daemon failed, not the caller.
            _ => WireError::Internal {
                message: e.to_string(),
            },
        }
    }
}

impl From<Error> for WireError {
    fn from(e: Error) -> WireError {
        WireError::from(&e)
    }
}
