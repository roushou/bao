//! The crate's error type. Library crates expose typed, branchable errors —
//! never an opaque `anyhow::Error` (that's a binary concern).

use std::path::PathBuf;

use thiserror::Error as ThisError;

use crate::{sandbox::SandboxKind, types::Status};

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("base64 error: {0}")]
    Base64(#[from] base64::DecodeError),
    /// portable-pty's error type is itself an opaque `anyhow::Error` (which
    /// does not implement `std::error::Error`), so we carry its message in a
    /// typed variant — consumers branch on `Error::Pty(_)` regardless.
    #[error("pty error: {0}")]
    Pty(String),
    #[error("the {0} transport is not wired up yet")]
    TransportUnsupported(&'static str),
    #[error("address must be host:port")]
    BadAddr,
    #[error("empty command")]
    EmptyCommand,
    #[error("session is not running (interrupted)")]
    NotRunning,
    #[error("session already has a running process")]
    AlreadyRunning,
    #[error("only interrupted sessions can be resumed (this one is {0})")]
    ResumeNotInterrupted(Status),
    #[error("illegal lifecycle transition: {0} → {1}")]
    IllegalTransition(Status, &'static str),
    #[error("no session matches '{0}'")]
    NotFound(String),
    #[error("'{0}' is ambiguous ({1} id(s), {2} name(s)) — be more specific")]
    Ambiguous(String, usize, usize),
    #[error("directory does not exist: {0}")]
    DirNotFound(String),
    #[error("not a git repository")]
    NotAGitRepo,
    #[error("isolation {0} is not available here (not a git repo, or the backend is missing)")]
    IsolationUnavailable(SandboxKind),
    #[error("unknown sandbox kind: {0:?} (expected inplace | worktree)")]
    BadSandboxKind(String),
    #[error("invalid hostname: {0:?}")]
    BadHostname(String),
    #[error("session meta uses format {0}, newer than this Bao supports")]
    NewerFormat(u32),
    #[error("worktree error: {0}")]
    Worktree(String),
    #[error("frame too large: {0} bytes")]
    FrameTooLarge(usize),
    #[error("failed to spawn: {0}")]
    Spawn(String),
    #[error("invalid path: {0}")]
    BadPath(PathBuf),
}
