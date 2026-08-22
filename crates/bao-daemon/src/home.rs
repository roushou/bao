//! The bao home layout — where the daemon keeps its on-disk state.

use std::path::{Path, PathBuf};

/// The bao home. Owns the directory layout the daemon and CLI share:
/// `sessions/` for session records and event logs, `workspaces/` for
/// materialized working copies.
#[derive(Clone, Debug)]
pub struct Home {
    root: PathBuf,
}

impl Home {
    /// A home rooted at `root` (creates nothing).
    pub fn new(root: &Path) -> Self {
        Home {
            root: root.to_path_buf(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `<home>/sessions` — session records and event logs.
    pub fn sessions_dir(&self) -> PathBuf {
        self.root.join("sessions")
    }

    /// `<home>/workspaces` — materialized working copies.
    pub fn workspaces_dir(&self) -> PathBuf {
        self.root.join("workspaces")
    }
}
