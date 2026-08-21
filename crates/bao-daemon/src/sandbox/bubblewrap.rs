//! The `bubblewrap` sandbox: confine the harness to its workspace with
//! Linux namespace isolation (`bwrap`).
//!
//! This module owns the bwrap invocation — the *profile* — and nothing else.
//! It is deliberately a plain module, not a `SandboxBackend` on its own: the
//! backend (`Bubblewrap`) also needs a working copy (a git worktree), which
//! lives in [`backends`](super::backends).

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use bao_core::sandbox::Workspace;
use portable_pty::CommandBuilder;

use crate::error::Error;

/// Whether `bwrap` is available on this machine. Used to decide whether the
/// backend is advertised, and to refuse (never silently degrade) a requested
/// bubblewrap session when it is missing.
pub(super) fn available() -> bool {
    std::process::Command::new("bwrap")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Rewrite a harness command to run inside bubblewrap. Keeps the command's
/// cwd and environment; only the argv and the controlling-tty flag change.
pub(super) fn wrap_command(workspace: &Workspace, cmd: &mut CommandBuilder) -> Result<(), Error> {
    let wrapped = wrap_argv(&workspace.path, cmd.get_argv());
    *cmd.get_argv_mut() = wrapped;
    // bwrap creates its own session inside the namespace; the outer spawn
    // must not try to assign the PTY as the controlling terminal (the same
    // workaround portable-pty documents for flatpak/container boundaries).
    cmd.set_controlling_tty(false);
    Ok(())
}

fn wrap_argv(workspace: &Path, argv: &[OsString]) -> Vec<OsString> {
    let mut out = bwrap_prefix(workspace);
    out.extend_from_slice(argv);
    out
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

#[cfg(test)]
mod tests {
    use bao_core::sandbox::SandboxKind;

    use super::*;

    #[test]
    fn wraps_argv_with_bwrap_and_preserves_command() {
        let dir = std::env::temp_dir().join(format!("bao-bwrap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let workspace = Workspace {
            kind: SandboxKind::Bubblewrap,
            repo: None,
            branch: None,
            path: dir.clone(),
        };
        let mut cmd = CommandBuilder::from_argv(
            ["bash", "-c", "echo hi"]
                .into_iter()
                .map(OsString::from)
                .collect(),
        );
        cmd.cwd(&dir);

        wrap_command(&workspace, &mut cmd).unwrap();

        let argv = cmd.get_argv();
        assert_eq!(argv[0].to_string_lossy(), "bwrap");
        let sep = argv.iter().position(|a| a == "--").expect("separator");
        let tail: Vec<String> = argv[sep + 1..]
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(tail, ["bash", "-c", "echo hi"]);
        assert!(!cmd.get_controlling_tty());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
