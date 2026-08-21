//! Sandbox strategies: the [`SandboxBackend`] port and its adapters. The
//! daemon is the only place that touches the OS to build or tear down a
//! [`Workspace`].
//!
//! This module holds the port and the store; the concrete adapters
//! (`InPlace`, `GitWorktree`) live in [`backends`].

use std::path::{Path, PathBuf};

use bao_core::{
    sandbox::{SandboxKind, SandboxSpec, Workspace},
    types::SessionId,
};
use portable_pty::CommandBuilder;

use crate::error::Error;

mod backends;
mod bubblewrap;

pub use backends::{Bubblewrap, GitWorktree, InPlace};

/// The capability to materialize and remove a [`Workspace`] — the launch
/// saga's forward and compensating steps. One adapter per isolation
/// mechanism; a new one (bubblewrap/Landlock, Seatbelt, a container) is a new
/// impl — the saga and the FSM don't change.
pub trait SandboxBackend: Send + Sync {
    /// The isolation level this strategy provides.
    fn kind(&self) -> SandboxKind;
    /// Build the working copy (the saga's forward step).
    fn prepare(&self, id: &SessionId, cwd: &Path) -> Result<Workspace, Error>;
    /// Undo a prepared working copy (the saga's compensating step).
    fn compensate(&self, workspace: &Workspace) -> Result<(), Error>;
}

/// Owns the workspace store (`<home>/envs/`) and resolves a [`SandboxSpec`]
/// into a concrete [`Workspace`] — never silently delivering a weaker kind
/// than requested.
#[derive(Clone)]
pub struct SandboxStore {
    pub dir: PathBuf,
}

impl SandboxStore {
    pub fn new(dir: PathBuf) -> Self {
        std::fs::create_dir_all(&dir).ok();
        SandboxStore { dir }
    }

    /// Build a working copy for session `id` starting from `cwd`. A `spec` of
    /// `None` resolves the strongest the machine offers; a requested kind
    /// that cannot be provided is an error, never a silent downgrade.
    pub fn create(
        &self,
        id: &SessionId,
        cwd: &Path,
        spec: &SandboxSpec,
    ) -> Result<Workspace, Error> {
        match spec.isolation {
            Some(kind) => self.backend(kind).prepare(id, cwd),
            None => match self.backend(SandboxKind::Worktree).prepare(id, cwd) {
                Ok(s) => Ok(s),
                Err(_) => self.backend(SandboxKind::InPlace).prepare(id, cwd),
            },
        }
    }

    /// Undo a working copy (the saga's compensating step). In-place copies are
    /// the user's own directory and are left untouched.
    pub fn remove(&self, workspace: &Workspace) -> Result<(), Error> {
        self.backend(workspace.kind).compensate(workspace)
    }

    fn backend(&self, kind: SandboxKind) -> Box<dyn SandboxBackend> {
        match kind {
            SandboxKind::InPlace => Box::new(InPlace),
            SandboxKind::Worktree => Box::new(GitWorktree::new(self.dir.clone())),
            SandboxKind::Bubblewrap => Box::new(Bubblewrap::new(self.dir.clone())),
        }
    }
}

/// Rewrite a harness command to launch inside the sandbox for `kind`. A pure
/// function of the workspace record: the backend that *materialized* the
/// workspace is stateful (store dir, git), but the launch wrapper is not.
///
/// Non-sandboxed kinds leave the command unchanged.
pub(crate) fn wrap_command(
    kind: SandboxKind,
    workspace: &Workspace,
    cmd: &mut CommandBuilder,
) -> Result<(), Error> {
    match kind {
        SandboxKind::Bubblewrap => bubblewrap::wrap_command(workspace, cmd),
        SandboxKind::InPlace | SandboxKind::Worktree => Ok(()),
    }
}

/// The isolation backends this machine can actually provide, for the daemon's
/// self-description. A client offers only these, never more.
pub fn available_backends() -> Vec<SandboxKind> {
    let mut backends = vec![SandboxKind::InPlace, SandboxKind::Worktree];
    if bubblewrap::available() {
        backends.push(SandboxKind::Bubblewrap);
    }
    backends
}

#[cfg(test)]
mod tests {
    use std::{process::Command, str::FromStr};

    use super::*;

    fn temp(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bao-sandbox-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn git(args: &[&str], cwd: &Path) {
        let st = Command::new("git")
            .arg("-C")
            .arg(cwd)
            .args(args)
            .status()
            .unwrap();
        assert!(st.success(), "git {args:?} failed");
    }

    fn make_repo(root: &Path) -> PathBuf {
        let repo = root.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        git(&["init", "-q"], &repo);
        git(&["config", "user.email", "test@bao"], &repo);
        git(&["config", "user.name", "bao test"], &repo);
        std::fs::write(repo.join("main.txt"), "hello\n").unwrap();
        git(&["add", "."], &repo);
        git(&["commit", "-q", "-m", "init"], &repo);
        repo
    }

    #[test]
    fn worktree_is_isolated_and_removable() {
        let root = temp("worktree");
        let repo = make_repo(&root);
        let store = SandboxStore::new(root.join("envs"));
        let ws = store
            .create(
                &SessionId::from_str("abc12345").unwrap(),
                &repo,
                &SandboxSpec::default(),
            )
            .unwrap();
        assert!(ws.isolated());
        assert_ne!(ws.path, repo);
        assert_eq!(ws.branch.as_deref(), Some("bao-abc12345"));

        assert!(ws.path.join("main.txt").exists());
        std::fs::write(ws.path.join("marker.txt"), "x").unwrap();
        assert!(
            !repo.join("marker.txt").exists(),
            "main checkout must stay clean"
        );

        store.remove(&ws).unwrap();
        assert!(!ws.path.exists(), "worktree must be removed");
        let branches = Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["branch", "--list", "bao-abc12345"])
            .output()
            .unwrap();
        assert!(
            String::from_utf8_lossy(&branches.stdout).trim().is_empty(),
            "session branch must be deleted"
        );
        assert!(repo.join("main.txt").exists(), "repo untouched");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn non_git_dir_runs_in_place_and_is_left_alone() {
        let root = temp("inplace");
        let cwd = root.join("scratch");
        std::fs::create_dir_all(&cwd).unwrap();
        std::fs::write(cwd.join("notes.txt"), "mine").unwrap();
        let store = SandboxStore::new(root.join("envs"));
        let ws = store
            .create(
                &SessionId::from_str("deadbeef").unwrap(),
                &cwd,
                &SandboxSpec::default(),
            )
            .unwrap();
        assert!(!ws.isolated());
        assert_eq!(ws.path, cwd);
        store.remove(&ws).unwrap();
        assert!(cwd.join("notes.txt").exists(), "in-place dir untouched");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn requested_worktree_outside_a_repo_is_an_error() {
        let root = temp("requested");
        let cwd = root.join("scratch");
        std::fs::create_dir_all(&cwd).unwrap();
        let store = SandboxStore::new(root.join("envs"));
        let spec = SandboxSpec {
            isolation: Some(SandboxKind::Worktree),
        };
        assert!(matches!(
            store
                .create(&SessionId::from_str("dead0001").unwrap(), &cwd, &spec)
                .unwrap_err(),
            Error::SandboxUnavailable(SandboxKind::Worktree)
        ));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
