//! The on-disk store of materialized working copies.

use std::path::{Path, PathBuf};

use bao_core::types::SessionId;

/// Owns the directory where the daemon materializes each session's working
/// copy (`<home>/workspaces`) — and the layout inside it. This is the store
/// primitive the sandbox backends place their checkouts into; it persists no
/// records (session identity lives in the session store), only locations.
#[derive(Clone, Debug)]
pub struct WorkspaceStore {
    dir: PathBuf,
}

impl WorkspaceStore {
    pub(crate) fn new(dir: PathBuf) -> Self {
        std::fs::create_dir_all(&dir).ok();
        WorkspaceStore { dir }
    }

    /// The store root.
    pub fn root(&self) -> &Path {
        &self.dir
    }

    /// `<root>/<id>/tree` — where a session's worktree checkout lives.
    pub fn tree_dir(&self, id: &SessionId) -> PathBuf {
        self.dir.join(id.as_str()).join("tree")
    }
}
