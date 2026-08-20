//! Sandbox strategies: the [`Sandbox`] port and its adapters. The daemon is
//! the only place that touches the OS to build or tear down a [`Workspace`].

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use bao_core::{
    error::Error,
    sandbox::{SandboxKind, SandboxSpec, Workspace},
    types::SessionId,
};

/// The capability to materialize and remove a [`Workspace`] — the launch
/// saga's forward and compensating steps. One adapter per isolation
/// mechanism; a new one (bubblewrap/Landlock, Seatbelt, a container) is a new
/// impl — the saga and the FSM don't change.
pub trait Sandbox: Send + Sync {
    /// The isolation level this strategy provides.
    fn kind(&self) -> SandboxKind;
    /// Build the working copy (the saga's forward step).
    fn prepare(&self, id: &SessionId, cwd: &Path) -> Result<Workspace, Error>;
    /// Undo a prepared working copy (the saga's compensating step).
    fn compensate(&self, workspace: &Workspace) -> Result<(), Error>;
}

/// Runs the session in the user's own directory — no isolation.
pub struct InPlace;

impl Sandbox for InPlace {
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
    fn new(dir: PathBuf) -> Self {
        Self { dir }
    }
}

impl Sandbox for GitWorktree {
    fn kind(&self) -> SandboxKind {
        SandboxKind::Worktree
    }

    fn prepare(&self, id: &SessionId, cwd: &Path) -> Result<Workspace, Error> {
        let repo = git_root(cwd).map_err(|_| Error::IsolationUnavailable(SandboxKind::Worktree))?;
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

    fn backend(&self, kind: SandboxKind) -> Box<dyn Sandbox> {
        match kind {
            SandboxKind::InPlace => Box::new(InPlace),
            SandboxKind::Worktree => Box::new(GitWorktree::new(self.dir.clone())),
        }
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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

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
            Error::IsolationUnavailable(SandboxKind::Worktree)
        ));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
