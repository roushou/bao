//! Sandbox strategies: the [`SandboxBackend`] port and its adapters. The
//! daemon is the only place that touches the OS to build or tear down a
//! [`Workspace`].
//!
//! This module holds the port and the store; each adapter (`InPlace`,
//! `GitWorktree`, `Bubblewrap`) lives in its own file.

use std::path::{Path, PathBuf};

use bao_core::{
    sandbox::{SandboxKind, SandboxSpec, Workspace},
    types::SessionId,
};
use portable_pty::CommandBuilder;

use crate::error::Error;

mod bubblewrap;
mod inplace;
mod worktree;

pub use bubblewrap::Bubblewrap;
pub use inplace::InPlace;
pub use worktree::GitWorktree;

/// A materialized sandbox: the serializable [`Workspace`] plus the backend
/// that created it (which may hold runtime state — a container id, a VM
/// socket — invisible to the domain).
#[derive(Debug)]
pub struct Sandbox {
    pub workspace: Workspace,
    backend: Box<dyn SandboxBackend>,
}

impl Sandbox {
    pub(crate) fn new(workspace: Workspace, backend: Box<dyn SandboxBackend>) -> Self {
        Sandbox { workspace, backend }
    }

    /// Rewrite the harness command to launch inside this sandbox.
    pub fn wrap_command(&self, cmd: &mut CommandBuilder) -> Result<(), Error> {
        self.backend.wrap_command(&self.workspace, cmd)
    }

    /// Tear the sandbox down (the launch saga's compensating step).
    pub fn teardown(&self) -> Result<(), Error> {
        self.backend.teardown(&self.workspace)
    }
}

/// The capability to materialize, launch-in, and tear down a [`Workspace`].
/// One adapter per isolation mechanism; a new one (Landlock, Seatbelt, a
/// container) is a new impl — the saga and the FSM don't change.
pub trait SandboxBackend: Send + Sync + std::fmt::Debug {
    /// The isolation level this strategy provides.
    fn kind(&self) -> SandboxKind;
    /// Materialize the working copy.
    fn prepare(&self, id: &SessionId, cwd: &Path) -> Result<Workspace, Error>;
    /// Rewrite the harness command for launch inside this sandbox.
    /// Default: launch unchanged.
    fn wrap_command(&self, workspace: &Workspace, cmd: &mut CommandBuilder) -> Result<(), Error> {
        let _ = (workspace, cmd);
        Ok(())
    }
    /// Undo [`Self::prepare`].
    fn teardown(&self, workspace: &Workspace) -> Result<(), Error>;
}

/// Owns the workspace store (`<home>/envs/`) and resolves a [`SandboxSpec`]
/// into a concrete [`Sandbox`] — never silently delivering a weaker kind
/// than requested.
#[derive(Clone)]
pub struct SandboxStore {
    pub dir: PathBuf,
    #[cfg(test)]
    override_backend: Option<fn() -> Box<dyn SandboxBackend>>,
}

impl SandboxStore {
    pub fn new(dir: PathBuf) -> Self {
        std::fs::create_dir_all(&dir).ok();
        SandboxStore {
            dir,
            #[cfg(test)]
            override_backend: None,
        }
    }

    /// Materialize a sandbox for `spec`, retaining its backend for spawn and
    /// teardown. A `spec` of `None` resolves the strongest the machine
    /// offers; a requested kind that cannot be provided is an error, never a
    /// silent downgrade.
    pub fn create(&self, id: &SessionId, cwd: &Path, spec: &SandboxSpec) -> Result<Sandbox, Error> {
        match spec.isolation {
            Some(kind) => self.prepare_with(kind, id, cwd),
            None => {
                let backend = self.backend(SandboxKind::Worktree);
                match backend.prepare(id, cwd) {
                    Ok(workspace) => Ok(Sandbox::new(workspace, backend)),
                    Err(_) => self.prepare_with(SandboxKind::InPlace, id, cwd),
                }
            }
        }
    }

    /// Re-create a backend from a persisted workspace (restore/resume).
    pub fn backend_for(&self, kind: SandboxKind) -> Box<dyn SandboxBackend> {
        self.backend(kind)
    }

    fn prepare_with(
        &self,
        kind: SandboxKind,
        id: &SessionId,
        cwd: &Path,
    ) -> Result<Sandbox, Error> {
        let backend = self.backend(kind);
        let workspace = backend.prepare(id, cwd)?;
        Ok(Sandbox::new(workspace, backend))
    }

    fn backend(&self, kind: SandboxKind) -> Box<dyn SandboxBackend> {
        #[cfg(test)]
        if let Some(f) = self.override_backend {
            return (f)();
        }
        match kind {
            SandboxKind::InPlace => Box::new(InPlace),
            SandboxKind::Worktree => Box::new(GitWorktree::new(self.dir.clone())),
            SandboxKind::Bubblewrap => Box::new(Bubblewrap::new(self.dir.clone())),
        }
    }
}

/// The isolation backends this machine can actually provide, for the daemon's
/// self-description. A client offers only these, never more.
pub fn available_backends() -> Vec<SandboxKind> {
    #[allow(unused_mut)]
    let mut backends = vec![SandboxKind::InPlace, SandboxKind::Worktree];
    #[cfg(all(feature = "bubblewrap", target_os = "linux"))]
    if bubblewrap::bwrap_available() {
        backends.push(SandboxKind::Bubblewrap);
    }
    backends
}

#[cfg(test)]
impl SandboxStore {
    /// Inject a test backend, overriding every kind. Test-only.
    pub(crate) fn with_override(mut self, f: fn() -> Box<dyn SandboxBackend>) -> Self {
        self.override_backend = Some(f);
        self
    }
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
        let sandbox = store
            .create(
                &SessionId::from_str("abc12345").unwrap(),
                &repo,
                &SandboxSpec::default(),
            )
            .unwrap();
        let ws = &sandbox.workspace;
        assert!(ws.isolated());
        assert_ne!(ws.path, repo);
        assert_eq!(ws.branch.as_deref(), Some("bao-abc12345"));

        assert!(ws.path.join("main.txt").exists());
        std::fs::write(ws.path.join("marker.txt"), "x").unwrap();
        assert!(
            !repo.join("marker.txt").exists(),
            "main checkout must stay clean"
        );

        sandbox.teardown().unwrap();
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
        let sandbox = store
            .create(
                &SessionId::from_str("deadbeef").unwrap(),
                &cwd,
                &SandboxSpec::default(),
            )
            .unwrap();
        assert!(!sandbox.workspace.isolated());
        assert_eq!(sandbox.workspace.path, cwd);
        sandbox.teardown().unwrap();
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
