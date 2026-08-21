//! The domain's error type: the few rules that pure domain functions can
//! violate. Everything the daemon, transport, or client does is *not* here —
//! each of those crates owns its own error.

use thiserror::Error as ThisError;

use crate::types::Status;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("empty command")]
    EmptyCommand,
    #[error("invalid hostname: {0:?}")]
    BadHostname(String),
    #[error("unknown sandbox kind: {0:?} (expected inplace | worktree)")]
    BadSandboxKind(String),
    #[error("illegal lifecycle transition: {0} → {1}")]
    IllegalTransition(Status, &'static str),
}
