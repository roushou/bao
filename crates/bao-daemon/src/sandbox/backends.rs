//! The [`SandboxBackend`] adapters: one strategy per isolation mechanism.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use bao_core::{
    sandbox::{SandboxKind, Workspace},
    types::SessionId,
};

use crate::error::Error;

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
        let repo = git_root(cwd).map_err(|_| Error::SandboxUnavailable(SandboxKind::Worktree))?;
        let path = self.dir.join(id.as_str()).join("tree");
        let branch = format!("bao-{id}");
        let status = Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["worktree", "add", "-b", &branch])
            .arg(&path)
            .arg("HEAD")
            .status()
            .map_err(|e| Error::Worktree(format!("failed to run git worktree add: {e}")))?;
        if !status.success() {
            return Err(Error::Worktree(format!(
                "could not create a worktree for session {id} in {} (branch `{branch}` may already exist)",
                repo.display()
            )));
        }
        Ok(Workspace {
            kind: SandboxKind::Worktree,
            repo: Some(repo),
            branch: Some(branch),
            path,
        })
    }

    fn compensate(&self, workspace: &Workspace) -> Result<(), Error> {
        let repo = workspace
            .repo
            .as_ref()
            .ok_or_else(|| Error::Worktree("worktree workspace without a repo root".to_string()))?;
        let _ = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["worktree", "remove", "--force"])
            .arg(&workspace.path)
            .status();
        if let Some(branch) = &workspace.branch {
            let _ = Command::new("git")
                .arg("-C")
                .arg(repo)
                .args(["branch", "-D", branch])
                .status();
        }
        // Best-effort: make sure the tree and its store dir are gone.
        let _ = std::fs::remove_dir_all(&workspace.path);
        if let Some(parent) = workspace.path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
        Ok(())
    }
}

fn git_root(cwd: &Path) -> Result<PathBuf, Error> {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--show-toplevel"])
        .output()?;
    if !out.status.success() {
        return Err(Error::NotAGitRepo);
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
    ))
}
