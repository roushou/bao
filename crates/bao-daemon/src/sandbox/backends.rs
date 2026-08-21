//! The [`SandboxBackend`] adapters: one strategy per isolation mechanism.

use std::path::{Path, PathBuf};

use bao_core::{
    sandbox::{SandboxKind, Workspace},
    types::SessionId,
};

use crate::{error::Error, git::Git};

use super::{SandboxBackend, bubblewrap};

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
        if workspace.repo.is_none() {
            return Err(Error::Worktree(
                "worktree workspace without a repo root".to_string(),
            ));
        }
        teardown_worktree(workspace);
        Ok(())
    }
}

/// A `bubblewrap` namespace sandbox, backed by a git worktree when the launch
/// directory is inside a repo (the worktree is the working copy; bwrap is the
/// confinement).
pub struct Bubblewrap {
    dir: PathBuf,
}

impl Bubblewrap {
    pub(super) fn new(dir: PathBuf) -> Self {
        Self { dir }
    }
}

impl SandboxBackend for Bubblewrap {
    fn kind(&self) -> SandboxKind {
        SandboxKind::Bubblewrap
    }

    fn prepare(&self, id: &SessionId, cwd: &Path) -> Result<Workspace, Error> {
        if !bubblewrap::available() {
            return Err(Error::SandboxUnavailable(SandboxKind::Bubblewrap));
        }
        // Working copy: a git worktree when possible, else the user's dir —
        // the namespace confinement applies either way.
        let mut ws = GitWorktree::new(self.dir.clone())
            .prepare(id, cwd)
            .unwrap_or(Workspace {
                kind: SandboxKind::Bubblewrap,
                repo: None,
                branch: None,
                path: cwd.to_path_buf(),
            });
        ws.kind = SandboxKind::Bubblewrap;
        Ok(ws)
    }

    fn compensate(&self, workspace: &Workspace) -> Result<(), Error> {
        // bwrap is per-process, so there is nothing to tear down beyond the
        // working copy.
        teardown_worktree(workspace);
        Ok(())
    }
}

/// Best-effort teardown of a worktree-backed working copy. A no-op for
/// in-place workspaces (their directory is the user's own and is never
/// removed).
fn teardown_worktree(workspace: &Workspace) {
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
