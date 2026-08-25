//! The workspace — what a launch is aimed at. A user-declared named root
//! (`myapp` → `~/dev/myapp`) that may contain any number of directories and
//! repos; sessions are targeted at workspaces, not at wherever the shell
//! happens to be. The registry that persists these lives in the daemon
//! ([`bao_daemon::workspace`]); this crate holds only the record and its
//! rules.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Error;

/// The longest alias Bao accepts — long enough to be a name, short enough
/// to type as a command argument.
const MAX_ALIAS: usize = 64;

/// A registered launch target: a stable alias plus the root path it names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    /// The client-facing handle (`bao launch myapp`). Unique per host.
    pub alias: String,
    /// The root directory sessions in this workspace run from.
    pub root: PathBuf,
}

impl Workspace {
    /// A validated workspace record. The alias must be typable: non-empty,
    /// no whitespace or path separators, at most [`MAX_ALIAS`] chars.
    pub fn new(alias: impl Into<String>, root: impl Into<PathBuf>) -> Result<Self, Error> {
        let alias = alias.into();
        let bad = |a: &str| Err(Error::BadAlias(a.to_string()));
        if alias.is_empty() || alias.chars().count() > MAX_ALIAS {
            return bad(&alias);
        }
        if alias
            .chars()
            .any(|c| c.is_whitespace() || c == '/' || c == '\\' || c.is_control())
        {
            return bad(&alias);
        }
        Ok(Workspace {
            alias,
            root: root.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_typable_aliases() {
        for alias in ["myapp", "a", "web-api", "under_score", "2nd.project"] {
            assert!(Workspace::new(alias, "/tmp/x").is_ok(), "{alias}");
        }
    }

    #[test]
    fn rejects_untypable_aliases() {
        for alias in [
            "",
            " ",
            "has space",
            "slash/ed",
            "back\\slash",
            "tab\there",
            &"x".repeat(MAX_ALIAS + 1),
        ] {
            assert!(Workspace::new(alias, "/tmp/x").is_err(), "{alias:?}");
        }
    }

    #[test]
    fn record_is_serializable() {
        let ws = Workspace::new("myapp", "/home/u/dev/myapp").unwrap();
        let json = serde_json::to_string(&ws).unwrap();
        let back: Workspace = serde_json::from_str(&json).unwrap();
        assert_eq!(ws, back);
    }
}
