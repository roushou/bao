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
}

impl fmt::Display for SandboxKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            SandboxKind::InPlace => "inplace",
            SandboxKind::Worktree => "worktree",
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
            other => Err(Error::BadSandboxKind(other.to_string())),
        }
    }
}

/// What a launch asks for in isolation. The daemon resolves this into a
/// concrete [`Workspace`] and never silently degrades it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxSpec {
    /// Requested isolation. `None` = resolve the strongest this machine
    /// offers for the launch cwd (git repo → worktree, else in place).
    pub isolation: Option<SandboxKind>,
}

/// The isolated working copy an session runs in: where, its git identity (if
/// any), and the isolation claim. Serialized into `meta.json` and shipped on
/// the wire; this is the object the move slice will pack.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workspace {
    pub kind: SandboxKind,
    /// Repo root (worktree sandboxes only).
    pub repo: Option<PathBuf>,
    /// Session branch (worktree sandboxes only).
    pub branch: Option<String>,
    /// The working copy the session runs in.
    pub path: PathBuf,
}

impl Workspace {
    pub fn isolated(&self) -> bool {
        self.kind == SandboxKind::Worktree
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Workspace {
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
        assert!(SandboxKind::from_str("container").is_err());
        assert_eq!(SandboxKind::InPlace.to_string(), "inplace");
    }
}
