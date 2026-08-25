//! The git-worktree backend: an isolated checkout on its own branch.

use std::path::Path;

use bao_core::{
    sandbox::{SandboxKind, WorkingCopy},
    types::SessionId,
};

use crate::{error::Error, git::Git};

use super::{SandboxBackend, WorkingCopyStore};

/// A `git worktree` on its own branch, owned by Bao.
#[derive(Debug)]
pub struct GitWorktree {
    store: WorkingCopyStore,
}

impl GitWorktree {
    pub(super) fn new(store: WorkingCopyStore) -> Self {
        Self { store }
    }
}

impl SandboxBackend for GitWorktree {
    fn kind(&self) -> SandboxKind {
        SandboxKind::Worktree
    }

    fn prepare(&self, id: &SessionId, cwd: &Path) -> Result<WorkingCopy, Error> {
        let git =
            Git::discover(cwd).map_err(|_| Error::SandboxUnavailable(SandboxKind::Worktree))?;
        let path = self.store.tree_dir(id);
        let branch = format!("bao-{id}");
        git.worktree_add(&branch, &path, "HEAD").map_err(|e| {
            Error::Worktree(format!("could not create a worktree for session {id}: {e}"))
        })?;
        Ok(WorkingCopy {
            kind: SandboxKind::Worktree,
            repo: Some(git.root().to_path_buf()),
            branch: Some(branch),
            path,
        })
    }

    fn teardown(&self, working_copy: &WorkingCopy) -> Result<(), Error> {
        if working_copy.repo.is_none() {
            return Err(Error::Worktree(
                "worktree working_copy without a repo root".to_string(),
            ));
        }
        teardown_worktree(working_copy);
        Ok(())
    }
}

/// Best-effort teardown of a worktree-backed working copy. A no-op for
/// in-place working copies (their directory is the user's own and is never
/// removed).
pub(super) fn teardown_worktree(working_copy: &WorkingCopy) {
    let Some(repo) = &working_copy.repo else {
        return;
    };
    let git = Git::open(repo.clone());
    let _ = git.worktree_remove(&working_copy.path);
    if let Some(branch) = &working_copy.branch {
        let _ = git.branch_delete(branch);
    }
    let _ = std::fs::remove_dir_all(&working_copy.path);
    if let Some(parent) = working_copy.path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}
