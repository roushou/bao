//! The bubblewrap backend: a Linux namespace sandbox (`bwrap`), gated behind
//! the `bubblewrap` feature and `target_os = "linux"`. Everywhere else the
//! backend is a stub that refuses honestly.

use std::path::{Path, PathBuf};

use bao_core::{
    sandbox::{SandboxKind, Workspace},
    types::SessionId,
};

use crate::error::Error;

use super::SandboxBackend;

#[cfg(all(feature = "bubblewrap", target_os = "linux"))]
use super::worktree::{GitWorktree, teardown_worktree};

#[cfg(all(feature = "bubblewrap", target_os = "linux"))]
use portable_pty::CommandBuilder;
#[cfg(all(feature = "bubblewrap", target_os = "linux"))]
use std::{ffi::OsString, process::Command, sync::OnceLock};

/// A `bubblewrap` namespace sandbox, backed by a git worktree when the launch
/// directory is inside a repo (the worktree is the working copy; bwrap is the
/// confinement).
#[cfg(all(feature = "bubblewrap", target_os = "linux"))]
#[derive(Debug)]
pub struct Bubblewrap {
    dir: PathBuf,
}

#[cfg(all(feature = "bubblewrap", target_os = "linux"))]
impl Bubblewrap {
    pub(super) fn new(dir: PathBuf) -> Self {
        Self { dir }
    }
}

#[cfg(all(feature = "bubblewrap", target_os = "linux"))]
impl SandboxBackend for Bubblewrap {
    fn kind(&self) -> SandboxKind {
        SandboxKind::Bubblewrap
    }

    fn prepare(&self, id: &SessionId, cwd: &Path) -> Result<Workspace, Error> {
        if !bwrap_available() {
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

    fn wrap_command(&self, workspace: &Workspace, cmd: &mut CommandBuilder) -> Result<(), Error> {
        let wrapped = wrap_argv(&workspace.path, cmd.get_argv());
        *cmd.get_argv_mut() = wrapped;
        // bwrap creates its own session inside the namespace; the outer spawn
        // must not try to assign the PTY as the controlling terminal.
        cmd.set_controlling_tty(false);
        Ok(())
    }

    fn teardown(&self, workspace: &Workspace) -> Result<(), Error> {
        // bwrap is per-process, so there is nothing to tear down beyond the
        // working copy.
        teardown_worktree(workspace);
        Ok(())
    }
}

/// `SandboxKind::Bubblewrap` stays defined everywhere (it is core data); off
/// Linux or without the feature the backend refuses honestly instead of
/// silently downgrading.
#[cfg(not(all(feature = "bubblewrap", target_os = "linux")))]
#[derive(Debug)]
pub struct Bubblewrap;

#[cfg(not(all(feature = "bubblewrap", target_os = "linux")))]
impl Bubblewrap {
    pub(super) fn new(_dir: PathBuf) -> Self {
        Self
    }
}

#[cfg(not(all(feature = "bubblewrap", target_os = "linux")))]
impl SandboxBackend for Bubblewrap {
    fn kind(&self) -> SandboxKind {
        SandboxKind::Bubblewrap
    }

    fn prepare(&self, _id: &SessionId, _cwd: &Path) -> Result<Workspace, Error> {
        Err(Error::SandboxUnavailable(SandboxKind::Bubblewrap))
    }

    fn teardown(&self, _workspace: &Workspace) -> Result<(), Error> {
        Ok(())
    }
}

/// Whether `bwrap` is installed and runnable. Cached once per process — this
/// is probed on every `Info` handshake, not just at startup.
#[cfg(all(feature = "bubblewrap", target_os = "linux"))]
pub(super) fn bwrap_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        Command::new("bwrap")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// The bwrap argv that sets up the sandbox, ending in `--` so the harness
/// command follows verbatim.
///
/// Confinement provided: user/pid/ipc/uts namespaces, a read-only system,
/// a private `/tmp`, and only the workspace (plus `$HOME`, for the harness's
/// own config) writable. Network is deliberately left enabled — the harness
/// is a cloud-LLM client and needs it.
///
/// Known first-cut limitation: `$HOME` is bound read-write in full, so the
/// sandbox does *not* yet hide secrets under `$HOME` (`.ssh`, `.aws`, …).
/// Tightening that is a harness-level concern — the harness must declare the
/// few state dirs it needs, and the rest of home can then be hidden.
#[cfg(all(feature = "bubblewrap", target_os = "linux"))]
fn bwrap_prefix(workspace: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "bwrap".into(),
        "--unshare-user-try".into(),
        "--unshare-pid".into(),
        "--unshare-ipc".into(),
        "--unshare-uts".into(),
        "--die-with-parent".into(),
        "--new-session".into(),
        "--proc".into(),
        "/proc".into(),
        "--dev".into(),
        "/dev".into(),
        "--tmpfs".into(),
        "/tmp".into(),
    ];
    for dir in [
        "/usr", "/etc", "/opt", "/nix", "/bin", "/sbin", "/lib", "/lib64",
    ] {
        ro_bind(&mut args, dir);
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        if home.is_dir() {
            args.push("--bind".into());
            args.push(home.as_os_str().to_owned());
            args.push(home.as_os_str().to_owned());
        }
    }

    let ws = workspace.to_string_lossy();
    args.push("--bind".into());
    args.push(ws.as_ref().into());
    args.push(ws.as_ref().into());
    args.push("--chdir".into());
    args.push(ws.as_ref().into());
    args.push("--".into());
    args
}

/// Add a read-only bind for a system directory, recreating it as a symlink
/// when the host path is one (modern usr-merged layouts: `/bin -> usr/bin`).
#[cfg(all(feature = "bubblewrap", target_os = "linux"))]
fn ro_bind(args: &mut Vec<OsString>, path: &str) {
    let host = Path::new(path);
    let Ok(meta) = std::fs::symlink_metadata(host) else {
        return;
    };
    if meta.file_type().is_symlink() {
        if let Ok(target) = std::fs::read_link(host) {
            args.push("--symlink".into());
            args.push(target.into_os_string());
            args.push(path.into());
        }
    } else {
        args.push("--ro-bind".into());
        args.push(path.into());
        args.push(path.into());
    }
}

#[cfg(all(feature = "bubblewrap", target_os = "linux"))]
fn wrap_argv(workspace: &Path, argv: &[OsString]) -> Vec<OsString> {
    let mut out = bwrap_prefix(workspace);
    out.extend_from_slice(argv);
    out
}

#[cfg(all(test, feature = "bubblewrap", target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn wraps_argv_with_bwrap_and_preserves_command() {
        let dir = std::env::temp_dir().join(format!("bao-bwrap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let argv = ["bash", "-c", "echo hi"]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
        let wrapped = wrap_argv(&dir, &argv);
        assert_eq!(wrapped[0].to_string_lossy(), "bwrap");
        let sep = wrapped.iter().position(|a| a == "--").expect("separator");
        let tail: Vec<String> = wrapped[sep + 1..]
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(tail, ["bash", "-c", "echo hi"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
