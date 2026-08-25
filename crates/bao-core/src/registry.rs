//! The registry record: named things launches consume. One namespace, two
//! payload kinds — a **workspace** names *where* a session runs (a root
//! directory), a **profile** names *what* runs (an argv preset). Both are
//! aliases resolved host-side; the store that persists them lives in the
//! daemon ([`crate`-level docs](../index.html)). See `docs/design/registry.md`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Error;

/// The longest alias Bao accepts — long enough to be a name, short enough
/// to type as a command argument.
const MAX_ALIAS: usize = 64;

/// The payload behind an alias. Closed enum: new launch parameters arrive
/// as new variants, not new registries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EntryKind {
    /// A launch target: sessions aimed here run at this root.
    Workspace { root: PathBuf },
    /// A launch preset: sessions launched with this alias run this argv.
    Profile { argv: Vec<String> },
}

/// One registry entry: a stable alias plus its payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryEntry {
    /// The client-facing handle (`bao launch myapp`, `--profile review`).
    /// Unique across kinds — one namespace.
    pub alias: String,
    #[serde(flatten)]
    pub kind: EntryKind,
}

impl RegistryEntry {
    /// A workspace entry. The alias must be typable; the root is validated
    /// by the store that materializes it (it must exist on that host).
    pub fn workspace(alias: impl Into<String>, root: impl Into<PathBuf>) -> Result<Self, Error> {
        let alias = Self::alias(alias.into())?;
        Ok(RegistryEntry {
            alias,
            kind: EntryKind::Workspace { root: root.into() },
        })
    }

    /// A profile entry. The argv must be non-empty; it is stored verbatim —
    /// the host that launches it interprets it.
    pub fn profile(alias: impl Into<String>, argv: Vec<String>) -> Result<Self, Error> {
        let alias = Self::alias(alias.into())?;
        if argv.is_empty() || argv[0].is_empty() {
            return Err(Error::EmptyCommand);
        }
        Ok(RegistryEntry {
            alias,
            kind: EntryKind::Profile { argv },
        })
    }

    fn alias(alias: String) -> Result<String, Error> {
        let bad = |_| Error::BadAlias(alias.clone());
        if alias.is_empty() || alias.chars().count() > MAX_ALIAS {
            return Err(bad(()));
        }
        if alias
            .chars()
            .any(|c| c.is_whitespace() || c == '/' || c == '\\' || c.is_control())
        {
            return Err(bad(()));
        }
        Ok(alias)
    }

    /// The workspace root, if this entry is a workspace.
    pub fn root(&self) -> Option<&PathBuf> {
        match &self.kind {
            EntryKind::Workspace { root } => Some(root),
            _ => None,
        }
    }

    /// The argv, if this entry is a profile.
    pub fn argv(&self) -> Option<&[String]> {
        match &self.kind {
            EntryKind::Profile { argv } => Some(argv),
            _ => None,
        }
    }

    pub fn is_workspace(&self) -> bool {
        matches!(self.kind, EntryKind::Workspace { .. })
    }

    pub fn is_profile(&self) -> bool {
        matches!(self.kind, EntryKind::Profile { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_typable_aliases() {
        for alias in ["myapp", "a", "web-api", "under_score", "2nd.project"] {
            assert!(RegistryEntry::workspace(alias, "/tmp/x").is_ok(), "{alias}");
        }
    }

    #[test]
    fn rejects_untypable_aliases_and_empty_argv() {
        for alias in [
            "",
            " ",
            "has space",
            "slash/ed",
            "back\\slash",
            "tab\there",
            &"x".repeat(MAX_ALIAS + 1),
        ] {
            assert!(
                RegistryEntry::workspace(alias, "/tmp/x").is_err(),
                "{alias:?}"
            );
        }
        assert!(RegistryEntry::profile("ok", vec![]).is_err());
        assert!(RegistryEntry::profile("ok", vec![String::new()]).is_err());
    }

    #[test]
    fn kind_accessors_and_serialization_round_trip() {
        let ws = RegistryEntry::workspace("myapp", "/home/u/dev/myapp").unwrap();
        assert!(ws.is_workspace());
        assert_eq!(
            ws.root().map(|p| p.as_path()),
            Some("/home/u/dev/myapp".as_ref())
        );

        let pf = RegistryEntry::profile("review", vec!["pi".into(), "--model".into(), "o".into()])
            .unwrap();
        assert!(pf.is_profile());
        assert_eq!(
            pf.argv(),
            Some(&["pi".to_string(), "--model".to_string(), "o".to_string()][..])
        );

        for e in [ws, pf] {
            let json = serde_json::to_string(&e).unwrap();
            let back: RegistryEntry = serde_json::from_str(&json).unwrap();
            assert_eq!(e, back);
        }
    }

    #[test]
    fn serialized_shape_is_tagged_and_flat() {
        let ws = RegistryEntry::workspace("a", "/t").unwrap();
        let json = serde_json::to_value(&ws).unwrap();
        assert_eq!(json["alias"], "a");
        assert_eq!(json["kind"], "workspace");
        assert_eq!(json["root"], "/t");
    }
}
