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

    /// Materialize the requested sandbox kind into `store`. An explicit kind
    /// that cannot be provided is an error — never a silent downgrade; a
    /// `None` request resolves to the strongest backend this machine can
    /// actually provide.
    pub fn create(
        store: &WorkingCopyStore,
        id: &SessionId,
        cwd: &Path,
        spec: &SandboxSpec,
    ) -> Result<Sandbox, Error> {
        // `None` = "best available": resolve the concrete kind before prepare
        // so the materialization path stays explicit (and honest).
        let kind = match spec.isolation {
            Some(kind) => kind,
            // InPlace/Worktree are unconditional table entries, so `Backend::best`
            // is always `Some`; the fallback is defensive only.
            None => Backend::best().unwrap_or(SandboxKind::InPlace),
        };
        prepare_with(store, kind, id, cwd)
    }

    /// Re-create a backend from a persisted working copy (restore/resume).
    pub(crate) fn from_workspace(store: &WorkingCopyStore, working_copy: WorkingCopy) -> Sandbox {
        let kind = working_copy.kind;
        Sandbox::new(working_copy, Backend::construct(store, kind))
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
    let backend = Backend::construct(store, kind);
    let working_copy = backend.prepare(id, cwd)?;
    Ok(Sandbox::new(working_copy, backend))
}

/// Whether bubblewrap is usable here: compiled in (Linux + `bubblewrap`
/// feature) and the `bwrap` binary present.
fn bubblewrap_available() -> bool {
    #[cfg(all(feature = "bubblewrap", target_os = "linux"))]
    {
        bubblewrap::bwrap_available()
    }
    #[cfg(not(all(feature = "bubblewrap", target_os = "linux")))]
    {
        false
    }
}

/// Whether Seatbelt is usable here: macOS and `sandbox-exec` present.
fn seatbelt_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        seatbelt::seatbelt_available()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// A sandbox backend this build can provide: its isolation level, how
/// strongly it isolates, how to probe its availability, and how to build the
/// operational [`SandboxBackend`] for it.
///
/// This is a backend's *capability* — distinct from [`SandboxBackend`], the
/// operational interface of one that is materialized. `probe` carries both
/// the platform gate (Seatbelt is never available on Linux, Bubblewrap never
/// on macOS) and the runtime gate (is the binary there); `strength` orders
/// them so "best available" is `max_by_key(strength)`; `construct` builds the
/// backend — off-platform backends are stubs that refuse honestly on
/// `prepare`.
pub(crate) struct Backend {
    kind: SandboxKind,
    /// Higher = stronger isolation. Bubblewrap and Seatbelt share `2` because
    /// they are mutually exclusive by platform, so they never compete.
    strength: u8,
    probe: fn() -> bool,
    construct: fn(&WorkingCopyStore) -> Box<dyn SandboxBackend>,
}

impl Backend {
    /// The capability table — the single source of truth for what this build
    /// can provide. Every entry is always present (off-platform backends are
    /// honest stubs); `probe` decides what's advertised, `construct` decides
    /// what's built.
    fn all() -> Vec<Backend> {
        vec![
            Backend {
                kind: SandboxKind::InPlace,
                strength: 0,
                probe: || true,
                construct: |_| Box::new(InPlace) as Box<dyn SandboxBackend>,
            },
            Backend {
                kind: SandboxKind::Worktree,
                strength: 1,
                probe: || true,
                construct: |store| {
                    Box::new(GitWorktree::new(store.clone())) as Box<dyn SandboxBackend>
                },
            },
            Backend {
                kind: SandboxKind::Bubblewrap,
                strength: 2,
                probe: bubblewrap_available,
                construct: |store| {
                    Box::new(Bubblewrap::new(store.clone())) as Box<dyn SandboxBackend>
                },
            },
            Backend {
                kind: SandboxKind::Seatbelt,
                strength: 2,
                probe: seatbelt_available,
                construct: |store| {
                    Box::new(Seatbelt::new(store.clone())) as Box<dyn SandboxBackend>
                },
            },
        ]
    }

    /// The isolation levels this machine can actually provide, for the
    /// daemon's self-description. A client offers only these, never more.
    pub(crate) fn available() -> Vec<SandboxKind> {
        Self::all()
            .into_iter()
            .filter(|b| (b.probe)())
            .map(|b| b.kind)
            .collect()
    }

    /// The strongest isolation this machine can actually provide — the
    /// default when a launch asks for "best available". Always `Some`:
    /// `InPlace` and `Worktree` need no binary and are unconditional entries.
    pub(crate) fn best() -> Option<SandboxKind> {
        Self::all()
            .into_iter()
            .filter(|b| (b.probe)())
            .max_by_key(|b| b.strength)
            .map(|b| b.kind)
    }

    /// Build the operational [`SandboxBackend`] for `kind` against `store`.
    /// Every [`SandboxKind`] has an entry (off-platform ones are honest
    /// stubs), so this never misses.
    pub(crate) fn construct(
        store: &WorkingCopyStore,
        kind: SandboxKind,
    ) -> Box<dyn SandboxBackend> {
        Self::all()
            .into_iter()
            .find(|b| b.kind == kind)
            .map(|b| (b.construct)(store))
            .expect("every SandboxKind has a table entry")
    }
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
            &SandboxSpec {
                isolation: Some(SandboxKind::Worktree),
            },
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
                isolation: Some(SandboxKind::InPlace),
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
            isolation: Some(SandboxKind::Worktree),
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

    #[test]
    fn backend_available_and_best() {
        let available = Backend::available();
        // The two unconditional backends are always offered.
        assert!(available.contains(&SandboxKind::InPlace));
        assert!(available.contains(&SandboxKind::Worktree));

        // Best is always present, and never weaker than a worktree: a worktree
        // is always available, so "no isolation" can never be the best.
        let best = Backend::best().expect("InPlace/Worktree are unconditional");
        assert!(available.contains(&best));
        assert!(
            matches!(
                best,
                SandboxKind::Worktree | SandboxKind::Bubblewrap | SandboxKind::Seatbelt
            ),
            "best must be at least worktree-strength: {best}"
        );
    }

    #[test]
    fn construct_covers_every_kind() {
        let root = temp("construct");
        let store = WorkingCopyStore::new(root.clone());
        for kind in [
            SandboxKind::InPlace,
            SandboxKind::Worktree,
            SandboxKind::Bubblewrap,
            SandboxKind::Seatbelt,
        ] {
            let backend = Backend::construct(&store, kind);
            assert_eq!(backend.kind(), kind);
        }
        std::fs::remove_dir_all(&root).unwrap();
    }
}
