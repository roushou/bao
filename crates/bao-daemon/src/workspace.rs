//! The workspace registry — the daemon-side store of registered launch
//! targets. One JSON file per host (`<home>/workspaces.json`); a path only
//! means something on the host that can see it, so registration is per-host
//! by design (see `docs/design/workspaces.md`).

use std::path::{Path, PathBuf};

use bao_core::workspace::Workspace;

use crate::error::Error;

/// The registered workspaces of one host, persisted at `<home>/workspaces.json`.
#[derive(Debug, Clone)]
pub struct WorkspaceRegistry {
    /// Where to persist; `None` = in-memory (tests).
    path: Option<PathBuf>,
    entries: Vec<Workspace>,
}

impl WorkspaceRegistry {
    /// Load the host's registry from `<home>/workspaces.json` (an absent
    /// file is an empty registry, not an error).
    pub fn load(home_root: &Path) -> Result<Self, Error> {
        let path = home_root.join("workspaces.json");
        let entries = match std::fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str::<Vec<Workspace>>(&raw).map_err(Error::Json)?,
            Err(_) => Vec::new(),
        };
        Ok(WorkspaceRegistry {
            path: Some(path),
            entries,
        })
    }

    /// An empty registry that never persists (tests).
    pub fn in_memory() -> Self {
        WorkspaceRegistry {
            path: None,
            entries: Vec::new(),
        }
    }

    /// Register `alias` → `root`. The root must exist and be a directory;
    /// neither the alias nor the path may already be registered.
    pub fn add(&mut self, alias: &str, root: &Path) -> Result<Workspace, Error> {
        if self.entries.iter().any(|w| w.alias == alias) {
            return Err(Error::DuplicateWorkspace(alias.to_string()));
        }
        let root = root
            .canonicalize()
            .map_err(|e| Error::BadWorkspacePath(root.display().to_string(), e.to_string()))?;
        if !root.is_dir() {
            return Err(Error::BadWorkspacePath(
                root.display().to_string(),
                "not a directory".to_string(),
            ));
        }
        if let Some(w) = self.entries.iter().find(|w| w.root == root) {
            return Err(Error::WorkspacePathTaken(
                root.display().to_string(),
                w.alias.clone(),
            ));
        }
        let ws = Workspace::new(alias, root)?;
        self.entries.push(ws.clone());
        self.persist()?;
        Ok(ws)
    }

    /// Forget a workspace by alias. Sessions already launched against it are
    /// untouched — this only removes the launch target.
    pub fn remove(&mut self, alias: &str) -> Result<(), Error> {
        let before = self.entries.len();
        self.entries.retain(|w| w.alias != alias);
        if self.entries.len() == before {
            return Err(Error::UnknownWorkspace(alias.to_string()));
        }
        self.persist()
    }

    /// Resolve an alias to its registered root.
    pub fn resolve(&self, alias: &str) -> Option<&Workspace> {
        self.entries.iter().find(|w| w.alias == alias)
    }

    /// All registered workspaces, sorted by alias.
    pub fn list(&self) -> Vec<&Workspace> {
        let mut out: Vec<&Workspace> = self.entries.iter().collect();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bao-wsreg-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("app")).unwrap();
        dir
    }

    #[test]
    fn add_resolve_remove_roundtrip() {
        let mut reg = WorkspaceRegistry::in_memory();
        let root = temp_root("roundtrip");
        let ws = reg.add("myapp", &root.join("app")).unwrap();
        assert_eq!(ws.alias, "myapp");
        assert!(ws.root.ends_with("app"));
        assert_eq!(reg.resolve("myapp").unwrap().root, ws.root);
        assert_eq!(reg.list().len(), 1);
        reg.remove("myapp").unwrap();
        assert!(reg.resolve("myapp").is_none());
        assert!(reg.remove("myapp").is_err());
    }

    #[test]
    fn duplicate_alias_and_path_rejected() {
        let mut reg = WorkspaceRegistry::in_memory();
        let root = temp_root("dup");
        reg.add("app", &root.join("app")).unwrap();
        assert!(matches!(
            reg.add("app", &root.join("app")),
            Err(Error::DuplicateWorkspace(_))
        ));
        assert!(matches!(
            reg.add("again", &root.join("app")),
            Err(Error::WorkspacePathTaken(_, _))
        ));
    }

    #[test]
    fn missing_or_file_roots_rejected() {
        let mut reg = WorkspaceRegistry::in_memory();
        let root = temp_root("missing");
        assert!(reg.add("nope", &root.join("absent")).is_err());
        let file = root.join("file.txt");
        std::fs::write(&file, "x").unwrap();
        assert!(reg.add("file", &file).is_err());
    }

    #[test]
    fn persists_and_reloads() {
        let root = temp_root("persist");
        let mut reg = WorkspaceRegistry::load(&root).unwrap();
        reg.add("app", &root.join("app")).unwrap();
        let reloaded = WorkspaceRegistry::load(&root).unwrap();
        assert_eq!(
            reloaded.resolve("app").map(|w| w.root.clone()),
            reg.resolve("app").map(|w| w.root.clone())
        );
        // An absent file is an empty registry, not an error.
        let fresh = temp_root("fresh");
        assert!(WorkspaceRegistry::load(&fresh).unwrap().list().is_empty());
    }
}
