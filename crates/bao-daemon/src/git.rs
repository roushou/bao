//! A small git adapter: the only place this crate runs `git`.
//!
//! [`GitWorktree`](crate::sandbox::backends::GitWorktree) decides *what* to ask
//! git to do (worktree policy); this type decides *how* to ask (subprocess
//! plumbing, exit-status handling, error mapping). It is a plain struct, not
//! a trait — there is exactly one git implementation, and a port would be
//! speculative until a second one (libgit2, a test fake) exists.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::error::Error;

/// A handle to a git repository.
pub(crate) struct Git {
    root: PathBuf,
}

impl Git {
    /// Find the repository containing `cwd`. Fails with
    /// [`Error::NotAGitRepo`] when `cwd` is not inside a work tree.
    pub(crate) fn discover(cwd: &Path) -> Result<Git, Error> {
        let out = Command::new("git")
            .arg("-C")
            .arg(cwd)
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .map_err(|e| Error::Git(format!("failed to run `git rev-parse`: {e}")))?;
        if !out.status.success() {
            return Err(Error::NotAGitRepo);
        }
        Ok(Git {
            root: PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()),
        })
    }

    /// A handle to a known repository root (for teardown).
    pub(crate) fn open(root: PathBuf) -> Git {
        Git { root }
    }

    /// The repository root.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// `git worktree add -b <branch> <path> <target>`.
    pub(crate) fn worktree_add(
        &self,
        branch: &str,
        path: &Path,
        target: &str,
    ) -> Result<(), Error> {
        let path = path.to_string_lossy();
        self.run_ok(&["worktree", "add", "-b", branch, path.as_ref(), target])
    }

    /// `git worktree remove --force <path>`.
    pub(crate) fn worktree_remove(&self, path: &Path) -> Result<(), Error> {
        let path = path.to_string_lossy();
        self.run_ok(&["worktree", "remove", "--force", path.as_ref()])
    }

    /// `git branch -D <branch>`.
    pub(crate) fn branch_delete(&self, branch: &str) -> Result<(), Error> {
        self.run_ok(&["branch", "-D", branch])
    }

    /// Run a git command and require a zero exit status. A non-zero status
    /// carries the command's stderr in the error.
    fn run_ok(&self, args: &[&str]) -> Result<(), Error> {
        let out = self.run(args)?;
        if out.status.success() {
            return Ok(());
        }
        Err(Error::Git(format!(
            "`git {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        )))
    }

    fn run(&self, args: &[&str]) -> Result<std::process::Output, Error> {
        Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .output()
            .map_err(|e| Error::Git(format!("failed to run `git {}`: {e}", args.join(" "))))
    }
}
