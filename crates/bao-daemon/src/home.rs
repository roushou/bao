//! The bao home layout — where the daemon keeps its on-disk state.

use std::path::{Path, PathBuf};

/// The bao home. Owns the directory layout the daemon and CLI share:
/// `sessions/` for session records and event logs, `working-copies/` for
/// materialized working copies.
#[derive(Clone, Debug)]
pub struct Home {
    root: PathBuf,
}

impl Home {
    /// A home rooted at `root` (creates nothing beyond the one-shot layout
    /// migration).
    pub fn new(root: &Path) -> Self {
        let home = Home {
            root: root.to_path_buf(),
        };
        home.migrate();
        home
    }

    /// One-shot layout migration from before working copies had their own
    /// name: `<home>/workspaces` → `<home>/working-copies`. Runs only when
    /// the old dir exists and the new one doesn't; never touches data.
    fn migrate(&self) {
        let old = self.root.join("working_copies");
        let new = self.root.join("working-copies");
        if old.is_dir() && !new.exists() {
            std::fs::rename(&old, &new).ok();
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `<home>/sessions` — session records and event logs.
    pub fn sessions_dir(&self) -> PathBuf {
        self.root.join("sessions")
    }

    /// `<home>/working-copies` — materialized working copies.
    pub fn working_copies_dir(&self) -> PathBuf {
        self.root.join("working-copies")
    }
}
