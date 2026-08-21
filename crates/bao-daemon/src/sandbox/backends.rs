//! The [`SandboxBackend`] adapters: one strategy per isolation mechanism.

use std::path::{Path, PathBuf};

use bao_core::{
    sandbox::{SandboxKind, Workspace},
    types::SessionId,
};

use crate::{error::Error, git::Git};

use super::SandboxBackend;

/// Runs the session in the user's own directory — no isolation.
pub struct InPlace;

impl SandboxBackend for InPlace {
    fn kind(&self) -> SandboxKind {
        SandboxKind::InPlace
    }

    fn prepare(&self, _id: &SessionId, cwd: &Path) -> Result<Workspace, Error> {
        Ok(Workspace {
            kind: SandboxKind::InPlace,
            repo: None,
            branch: None,
            path: cwd.to_path_buf(),
        })
    }

    fn compensate(&self, _workspace: &Workspace) -> Result<(), Error> {
        // The user's own directory is never touched.
        Ok(())
    }
}

/// A `git worktree` on its own branch, owned by Bao.
pub struct GitWorktree {
    dir: PathBuf,
}

impl GitWorktree {
    pub(super) fn new(dir: PathBuf) -> Self {
        Self { dir }
    }
}

impl SandboxBackend for GitWorktree {
    fn kind(&self) -> SandboxKind {
        SandboxKind::Worktree
    }

    fn prepare(&self, id: &SessionId, cwd: &Path) -> Result<Workspace, Error> {
        let git =
            Git::discover(cwd).map_err(|_| Error::SandboxUnavailable(SandboxKind::Worktree))?;
        let path = self.dir.join(id.as_str()).join("tree");
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

    fn compensate(&self, workspace: &Workspace) -> Result<(), Error> {
        let repo = workspace
            .repo
            .as_ref()
            .ok_or_else(|| Error::Worktree("worktree workspace without a repo root".to_string()))?;
        let git = Git::open(repo.clone());
        // Best-effort: git already removed the tree, and the branch and the
        // store dir are cleaned up regardless.
        let _ = git.worktree_remove(&workspace.path);
        if let Some(branch) = &workspace.branch {
            let _ = git.branch_delete(branch);
        }
        let _ = std::fs::remove_dir_all(&workspace.path);
        if let Some(parent) = workspace.path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
        Ok(())
    }
}
