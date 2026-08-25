//! The registry: the one store of named things launches consume — workspace
//! targets and profile presets, one alias namespace. Persisted per host at
//! `<home>/registry.json` (a path only means something on the host that can
//! see it). Put is upsert: re-registering an alias replaces its entry.

use std::path::{Path, PathBuf};

use bao_core::registry::{EntryKind, RegistryEntry};

use crate::error::Error;

#[derive(Debug, Clone)]
pub struct Registry {
    /// Where to persist; `None` = in-memory (tests).
    path: Option<PathBuf>,
    entries: Vec<RegistryEntry>,
}

impl Registry {
    /// Load the host's registry from `<home>/registry.json`. On first load,
    /// folds in the legacy files (`workspaces.json`, `profiles.json`) and
    /// removes them after a successful write.
    pub fn load(home_root: &Path) -> Result<Self, Error> {
        let path = home_root.join("registry.json");
        match std::fs::read_to_string(&path) {
            Ok(raw) => Ok(Registry {
                entries: serde_json::from_str(&raw).map_err(Error::Json)?,
                path: Some(path),
            }),
            Err(_) => {
                let mut reg = Registry {
                    path: Some(path),
                    entries: Vec::new(),
                };
                let folded = reg.fold_legacy(home_root)?;
                if folded {
                    reg.persist()?;
                    let _ = std::fs::remove_file(home_root.join("workspaces.json"));
                    let _ = std::fs::remove_file(home_root.join("profiles.json"));
                }
                Ok(reg)
            }
        }
    }

    /// Fold legacy stores into `entries`. Returns whether anything was found.
    fn fold_legacy(&mut self, home_root: &Path) -> Result<bool, Error> {
        let mut folded = false;

        let ws_path = home_root.join("workspaces.json");
        if let Ok(raw) = std::fs::read_to_string(&ws_path)
            && let Ok(list) = serde_json::from_str::<Vec<LegacyWorkspace>>(&raw)
        {
            for w in list {
                if let Ok(e) = RegistryEntry::workspace(&w.alias, &w.root) {
                    self.entries.push(e);
                    folded = true;
                }
            }
        }

        let pf_path = home_root.join("profiles.json");
        if let Ok(raw) = std::fs::read_to_string(&pf_path)
            && let Some(obj) = serde_json::from_str::<serde_json::Value>(&raw)
                .ok()
                .and_then(|v| v.as_object().cloned())
        {
            for (name, val) in obj {
                let Some(cmd) = val.as_str() else { continue };
                // Unparseable legacy commands are skipped honestly, not guessed.
                let argv: Vec<String> = cmd.split_whitespace().map(String::from).collect();
                if let Ok(e) = RegistryEntry::profile(&name, argv) {
                    self.entries.push(e);
                    folded = true;
                }
            }
        }

        // The built-in default never migrated — it seeds itself below.
        Ok(folded)
    }

    /// An empty registry that never persists (tests).
    pub fn in_memory() -> Self {
        Registry {
            path: None,
            entries: Vec::new(),
        }
    }

    /// Insert or replace by alias (upsert). Validates per kind: a workspace
    /// root must exist on this host and no other workspace may claim it.
    pub fn put(&mut self, mut entry: RegistryEntry) -> Result<(), Error> {
        match &mut entry.kind {
            EntryKind::Workspace { root } => {
                *root = root.canonicalize().map_err(|e| {
                    Error::BadWorkspacePath(root.display().to_string(), e.to_string())
                })?;
                if !root.is_dir() {
                    return Err(Error::BadWorkspacePath(
                        root.display().to_string(),
                        "not a directory".to_string(),
                    ));
                }
                if let Some(other) = self.entries.iter().position(|e| {
                    e.is_workspace() && e.alias != entry.alias && e.root() == Some(root)
                }) {
                    return Err(Error::WorkspacePathTaken(
                        root.display().to_string(),
                        self.entries[other].alias.clone(),
                    ));
                }
            }
            EntryKind::Profile { .. } => {}
        }
        match self.entries.iter_mut().find(|e| e.alias == entry.alias) {
            Some(slot) => *slot = entry,
            None => self.entries.push(entry),
        }
        self.persist()
    }

    /// Forget an alias (any kind). Sessions already launched against it are
    /// untouched — this only removes the launch vocabulary.
    pub fn remove(&mut self, alias: &str) -> Result<(), Error> {
        let before = self.entries.len();
        self.entries.retain(|e| e.alias != alias);
        if self.entries.len() == before {
            return Err(Error::UnknownAlias(alias.to_string()));
        }
        self.persist()
    }

    /// Resolve an alias to a workspace root (launch targeting).
    pub fn resolve_workspace(&self, alias: &str) -> Option<PathBuf> {
        self.entries
            .iter()
            .find(|e| e.alias == alias && e.is_workspace())
            .and_then(|e| e.root().cloned())
    }

    /// Resolve an alias to a profile argv (launch preset).
    pub fn resolve_profile(&self, alias: &str) -> Option<Vec<String>> {
        self.entries
            .iter()
            .find(|e| e.alias == alias && e.is_profile())
            .and_then(|e| e.argv().map(|a| a.to_vec()))
    }

    /// All entries, sorted by alias.
    pub fn list(&self) -> Vec<&RegistryEntry> {
        let mut out: Vec<&RegistryEntry> = self.entries.iter().collect();
        out.sort_by(|a, b| a.alias.cmp(&b.alias));
        out
    }

    fn persist(&self) -> Result<(), Error> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(&self.entries)?;
        std::fs::write(path, raw)?;
        Ok(())
    }
}

/// Legacy `workspaces.json` row (pre-registry store).
#[derive(serde::Deserialize)]
struct LegacyWorkspace {
    alias: String,
    root: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bao-reg-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("app")).unwrap();
        dir
    }

    #[test]
    fn upsert_remove_and_resolve_by_kind() {
        let mut reg = Registry::in_memory();
        let root = temp_root("upsert");
        let ws = RegistryEntry::workspace("myapp", root.join("app")).unwrap();
        reg.put(ws).unwrap();

        // Upsert replaces; it does not duplicate or error.
        let moved = RegistryEntry::profile("myapp", vec!["pi".into()]).unwrap();
        reg.put(moved).unwrap();
        assert!(reg.resolve_workspace("myapp").is_none());
        assert_eq!(reg.resolve_profile("myapp"), Some(vec!["pi".to_string()]));
        assert_eq!(reg.list().len(), 1);

        reg.remove("myapp").unwrap();
        assert!(reg.resolve_profile("myapp").is_none());
        assert!(reg.remove("myapp").is_err());
    }

    #[test]
    fn workspace_validation_still_applies() {
        let mut reg = Registry::in_memory();
        let root = temp_root("validate");
        assert!(
            reg.put(RegistryEntry::workspace("nope", root.join("absent")).unwrap())
                .is_err()
        );
        reg.put(RegistryEntry::workspace("app", root.join("app")).unwrap())
            .unwrap();
        assert!(matches!(
            reg.put(RegistryEntry::workspace("again", root.join("app")).unwrap()),
            Err(Error::WorkspacePathTaken(_, _))
        ));
    }

    #[test]
    fn persists_and_reloads() {
        let root = temp_root("persist");
        let mut reg = Registry::load(&root).unwrap();
        reg.put(RegistryEntry::workspace("app", root.join("app")).unwrap())
            .unwrap();
        reg.put(RegistryEntry::profile("review", vec!["pi".into()]).unwrap())
            .unwrap();
        let reloaded = Registry::load(&root).unwrap();
        assert_eq!(reloaded.list().len(), 2);
        assert!(reloaded.resolve_workspace("app").is_some());
        assert!(reloaded.resolve_profile("review").is_some());
    }

    #[test]
    fn folds_legacy_files_and_removes_them() {
        use std::io::Write;
        let root = temp_root("legacy");
        std::fs::create_dir_all(root.join("backend")).unwrap();
        let mut ws = std::fs::File::create(root.join("workspaces.json")).unwrap();
        write!(
            ws,
            r#"[{{"alias":"app","root":"{}"}}]"#,
            root.join("backend").display()
        )
        .unwrap();
        let mut pf = std::fs::File::create(root.join("profiles.json")).unwrap();
        write!(pf, r#"{{"review": "pi --model fast", "broken": ""}}"#).unwrap();

        let reg = Registry::load(&root).unwrap();
        assert!(reg.resolve_workspace("app").is_some());
        assert_eq!(
            reg.resolve_profile("review").unwrap(),
            vec!["pi".to_string(), "--model".to_string(), "fast".to_string()]
        );
        assert!(
            reg.resolve_profile("broken").is_none(),
            "unparseable skipped"
        );
        assert!(!root.join("workspaces.json").exists());
        assert!(!root.join("profiles.json").exists());
        assert!(root.join("registry.json").exists());
    }
}
