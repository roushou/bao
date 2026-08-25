//! The sandbox *data* — what an isolated working copy is. The strategies
//! that materialize and remove one (`bao-daemon::sandbox::SandboxBackend`) live in
//! the daemon; this crate holds only the serializable record.

use std::{fmt, path::PathBuf, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::error::Error;

/// How a session's working copy is isolated. The kind *is* the isolation
/// claim — the overview surfaces it, never a stronger one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxKind {
    /// The user's own directory, used as-is — no isolation.
    InPlace,
    /// A `git worktree` on its own branch, owned by Bao.
    Worktree,
    /// A `bubblewrap` namespace sandbox: the harness runs in its own
    /// user/pid/ipc/uts namespaces with a read-only system, a private
    /// `/tmp`, and only its working copy writable. Stronger than a worktree;
    /// the working copy underneath is still a git worktree when the launch
    /// directory is inside a repo.
    Bubblewrap,
    /// A macOS Seatbelt sandbox (`sandbox-exec`): the harness runs under a
    /// profile that denies file writes except to the working copy, `$HOME`,
    /// `$TMPDIR`, and `/dev`. Reads, network, and subprocess spawning stay
    /// allowed. The working copy underneath is a git worktree when the
    /// launch directory is inside a repo.
    Seatbelt,
}

impl fmt::Display for SandboxKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            SandboxKind::InPlace => "inplace",
            SandboxKind::Worktree => "worktree",
            SandboxKind::Bubblewrap => "bubblewrap",
            SandboxKind::Seatbelt => "seatbelt",
        };
        write!(f, "{s}")
    }
}

impl FromStr for SandboxKind {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "inplace" => Ok(SandboxKind::InPlace),
            "worktree" => Ok(SandboxKind::Worktree),
            "bubblewrap" => Ok(SandboxKind::Bubblewrap),
            "seatbelt" => Ok(SandboxKind::Seatbelt),
            other => Err(Error::BadSandboxKind(other.to_string())),
        }
    }
}

/// What a launch asks for in isolation. The kind is always explicit: the
/// daemon materializes exactly this backend or fails — it never silently
/// downgrades or substitutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxSpec {
    /// Requested isolation.
    pub isolation: SandboxKind,
}

impl Default for SandboxSpec {
    /// The default backend: a git worktree (file isolation).
    fn default() -> Self {
        SandboxSpec {
            isolation: SandboxKind::Worktree,
        }
    }
}

/// The isolated working copy an session runs in: where, its git identity (if
/// any), and the isolation claim. Serialized into `meta.json` and shipped on
/// the wire; this is the object the move slice will pack.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkingCopy {
    pub kind: SandboxKind,
    /// Repo root (worktree-backed sandboxes only).
    pub repo: Option<PathBuf>,
    /// Session branch (worktree-backed sandboxes only).
    pub branch: Option<String>,
    /// The working copy the session runs in.
    pub path: PathBuf,
}

impl WorkingCopy {
    pub fn isolated(&self) -> bool {
        matches!(
            self.kind,
            SandboxKind::Worktree | SandboxKind::Bubblewrap | SandboxKind::Seatbelt
        )
    }
}

impl Default for WorkingCopy {
    fn default() -> Self {
        WorkingCopy {
            kind: SandboxKind::InPlace,
            repo: None,
            branch: None,
            path: PathBuf::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_kind_parses_and_displays() {
        assert_eq!(
            SandboxKind::from_str("inplace").unwrap(),
            SandboxKind::InPlace
        );
        assert_eq!(
            SandboxKind::from_str("worktree").unwrap(),
            SandboxKind::Worktree
        );
        assert_eq!(
            SandboxKind::from_str("bubblewrap").unwrap(),
            SandboxKind::Bubblewrap
        );
        assert_eq!(
            SandboxKind::from_str("seatbelt").unwrap(),
            SandboxKind::Seatbelt
        );
        assert!(SandboxKind::from_str("container").is_err());
        assert_eq!(SandboxKind::InPlace.to_string(), "inplace");
        assert_eq!(SandboxKind::Bubblewrap.to_string(), "bubblewrap");
        assert_eq!(SandboxKind::Seatbelt.to_string(), "seatbelt");
    }
}
