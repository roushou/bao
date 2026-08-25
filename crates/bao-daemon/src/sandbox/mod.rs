//! Sandbox strategies: the [`SandboxBackend`] port, the [`Sandbox`] handle,
//! the [`SandboxFactory`] seam, and the [`WorkingCopyStore`]. Each adapter
//! (`InPlace`, `GitWorktree`, `Bubblewrap`, `Seatbelt`) lives in its own file.

use std::path::Path;

use bao_core::{
    sandbox::{SandboxKind, SandboxSpec, WorkingCopy},
    types::SessionId,
};
use portable_pty::CommandBuilder;

use crate::error::Error;

mod bubblewrap;
mod inplace;
mod seatbelt;
mod store;
mod worktree;

#[cfg(test)]
pub(crate) mod fake;

pub use bubblewrap::Bubblewrap;
pub use inplace::InPlace;
pub use seatbelt::Seatbelt;
pub use store::WorkingCopyStore;
pub use worktree::GitWorktree;

/// A materialized sandbox: the serializable [`WorkingCopy`] plus the backend
/// that created it (which may hold runtime state — a container id, a VM
/// socket — invisible to the domain).
#[derive(Debug)]
pub struct Sandbox {
    pub working_copy: WorkingCopy,
    backend: Box<dyn SandboxBackend>,
}

impl Sandbox {
    pub(crate) fn new(working_copy: WorkingCopy, backend: Box<dyn SandboxBackend>) -> Self {
        Sandbox {
            working_copy,
            backend,
        }
    }

    /// Materialize the requested sandbox kind into `store`. A kind that
    /// cannot be provided is an error — never a silent downgrade.
    pub fn create(
        store: &WorkingCopyStore,
        id: &SessionId,
        cwd: &Path,
        spec: &SandboxSpec,
    ) -> Result<Sandbox, Error> {
        prepare_with(store, spec.isolation, id, cwd)
    }

    /// Re-create a backend from a persisted working copy (restore/resume).
    pub(crate) fn from_workspace(store: &WorkingCopyStore, working_copy: WorkingCopy) -> Sandbox {
        let kind = working_copy.kind;
        Sandbox::new(working_copy, backend(store, kind))
    }

    /// Rewrite the harness command to launch inside this sandbox.
    pub fn wrap_command(&self, cmd: &mut CommandBuilder) -> Result<(), Error> {
        self.backend.wrap_command(&self.working_copy, cmd)
    }

    /// Tear the sandbox down (the launch saga's compensating step).
    pub fn teardown(&self) -> Result<(), Error> {
        self.backend.teardown(&self.working_copy)
    }
}

/// Materializes a [`Sandbox`] for a launch. The production implementation
/// dispatches on the requested [`SandboxKind`]; tests inject a fake so the
/// registry and saga are testable without git, disk, or sandbox binaries.
pub trait SandboxFactory: Send + Sync {
    fn materialize(
        &self,
        store: &WorkingCopyStore,
        id: &SessionId,
        cwd: &Path,
        spec: &SandboxSpec,
    ) -> Result<Sandbox, Error>;
}

/// The production factory: materializes the real backend for the requested
/// kind.
#[derive(Debug, Default)]
pub struct RealSandboxFactory;

impl SandboxFactory for RealSandboxFactory {
    fn materialize(
        &self,
        store: &WorkingCopyStore,
        id: &SessionId,
        cwd: &Path,
        spec: &SandboxSpec,
    ) -> Result<Sandbox, Error> {
        Sandbox::create(store, id, cwd, spec)
    }
}

/// The capability to materialize, launch-in, and tear down a [`WorkingCopy`].
/// One adapter per isolation mechanism; a new one (Landlock, Seatbelt, a
/// container) is a new impl — the saga and the FSM don't change.
pub trait SandboxBackend: Send + Sync + std::fmt::Debug {
    /// The isolation level this strategy provides.
    fn kind(&self) -> SandboxKind;
    /// Materialize the working copy.
    fn prepare(&self, id: &SessionId, cwd: &Path) -> Result<WorkingCopy, Error>;
    /// Rewrite the harness command for launch inside this sandbox.
    /// Default: launch unchanged.
    fn wrap_command(
        &self,
        working_copy: &WorkingCopy,
        cmd: &mut CommandBuilder,
    ) -> Result<(), Error> {
        let _ = (working_copy, cmd);
        Ok(())
    }
    /// Undo [`Self::prepare`].
    fn teardown(&self, working_copy: &WorkingCopy) -> Result<(), Error>;
}

/// Materialize a sandbox of a specific kind.
fn prepare_with(
    store: &WorkingCopyStore,
    kind: SandboxKind,
    id: &SessionId,
    cwd: &Path,
) -> Result<Sandbox, Error> {
    let backend = backend(store, kind);
    let working_copy = backend.prepare(id, cwd)?;
    Ok(Sandbox::new(working_copy, backend))
}

/// Construct the backend for a kind.
fn backend(store: &WorkingCopyStore, kind: SandboxKind) -> Box<dyn SandboxBackend> {
    match kind {
        SandboxKind::InPlace => Box::new(InPlace),
        SandboxKind::Worktree => Box::new(GitWorktree::new(store.clone())),
        SandboxKind::Bubblewrap => Box::new(Bubblewrap::new(store.clone())),
        SandboxKind::Seatbelt => Box::new(Seatbelt::new(store.clone())),
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
    #[cfg(target_os = "macos")]
    if seatbelt::seatbelt_available() {
        backends.push(SandboxKind::Seatbelt);
    }
    backends
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, process::Command, str::FromStr};

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
        let store = WorkingCopyStore::new(root.join("working-copies"));
        let sandbox = Sandbox::create(
            &store,
            &SessionId::from_str("abc12345").unwrap(),
            &repo,
            &SandboxSpec::default(),
        )
        .unwrap();
        let ws = &sandbox.working_copy;
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
    fn in_place_runs_in_the_users_dir_and_is_untouched() {
        let root = temp("inplace");
        let cwd = root.join("scratch");
        std::fs::create_dir_all(&cwd).unwrap();
        std::fs::write(cwd.join("notes.txt"), "mine").unwrap();
        let store = WorkingCopyStore::new(root.join("working-copies"));
        let sandbox = Sandbox::create(
            &store,
            &SessionId::from_str("deadbeef").unwrap(),
            &cwd,
            &SandboxSpec {
                isolation: SandboxKind::InPlace,
            },
        )
        .unwrap();
        assert!(!sandbox.working_copy.isolated());
        assert_eq!(sandbox.working_copy.path, cwd);
        sandbox.teardown().unwrap();
        assert!(cwd.join("notes.txt").exists(), "in-place dir untouched");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn requested_worktree_outside_a_repo_is_an_error() {
        let root = temp("requested");
        let cwd = root.join("scratch");
        std::fs::create_dir_all(&cwd).unwrap();
        let store = WorkingCopyStore::new(root.join("working-copies"));
        let spec = SandboxSpec {
            isolation: SandboxKind::Worktree,
        };
        assert!(matches!(
            Sandbox::create(
                &store,
                &SessionId::from_str("dead0001").unwrap(),
                &cwd,
                &spec
            )
            .unwrap_err(),
            Error::SandboxUnavailable(SandboxKind::Worktree)
        ));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
