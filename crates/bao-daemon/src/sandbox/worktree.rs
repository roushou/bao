//! The git-worktree backend: an isolated checkout on its own branch.

use std::path::Path;

use bao_core::{
    sandbox::{SandboxKind, Workspace},
    types::SessionId,
};

use crate::{error::Error, git::Git};

use super::{SandboxBackend, WorkspaceStore};

/// A `git worktree` on its own branch, owned by Bao.
#[derive(Debug)]
pub struct GitWorktree {
    store: WorkspaceStore,
}

impl GitWorktree {
    pub(super) fn new(store: WorkspaceStore) -> Self {
        Self { store }
    }
}

impl SandboxBackend for GitWorktree {
    fn kind(&self) -> SandboxKind {
        SandboxKind::Worktree
    }

    fn prepare(&self, id: &SessionId, cwd: &Path) -> Result<Workspace, Error> {
        let git =
            Git::discover(cwd).map_err(|_| Error::SandboxUnavailable(SandboxKind::Worktree))?;
        let path = self.store.tree_dir(id);
        let branch = format!("bao-{id}");
        git.worktree_add(&branch, &path, "HEAD").map_err(|e| {
            Error::Worktree(format!("could not create a worktree for session {id}: {e}"))
        })?;
        Ok(Workspace {
            kind: SandboxKind::Worktree,
            repo: Some(git.root().to_path_buf()),
            branch: Some(branch),
            path,
        })
    }

    fn teardown(&self, workspace: &Workspace) -> Result<(), Error> {
        if workspace.repo.is_none() {
            return Err(Error::Worktree(
                "worktree workspace without a repo root".to_string(),
            ));
        }
        teardown_worktree(workspace);
        Ok(())
    }
}

/// Best-effort teardown of a worktree-backed working copy. A no-op for
/// in-place workspaces (their directory is the user's own and is never
/// removed).
pub(super) fn teardown_worktree(workspace: &Workspace) {
    let Some(repo) = &workspace.repo else {
        return;
    };
    let git = Git::open(repo.clone());
    let _ = git.worktree_remove(&workspace.path);
    if let Some(branch) = &workspace.branch {
        let _ = git.branch_delete(branch);
    }
    let _ = std::fs::remove_dir_all(&workspace.path);
    if let Some(parent) = workspace.path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}
