//! The Seatbelt backend: macOS's native sandbox (`sandbox-exec`), available
//! on `target_os = "macos"`. Everywhere else the backend is a stub that
//! refuses honestly.

use std::path::Path;

use bao_core::{
    sandbox::{SandboxKind, WorkingCopy},
    types::SessionId,
};

use crate::error::Error;

use super::{SandboxBackend, WorkingCopyStore};

#[cfg(target_os = "macos")]
use super::worktree::{GitWorktree, teardown_worktree};

#[cfg(target_os = "macos")]
use portable_pty::CommandBuilder;
#[cfg(target_os = "macos")]
use std::{ffi::OsString, path::PathBuf, process::Command, sync::OnceLock};

/// A Seatbelt sandbox over a git worktree (when the launch directory is in a
/// repo) or the user's directory otherwise. Seatbelt confines *writes* —
/// unlike a bare worktree, the process cannot write anywhere else on the
/// system. Reads, network, and subprocess spawning stay allowed.
#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct Seatbelt {
    store: WorkingCopyStore,
}

#[cfg(target_os = "macos")]
impl Seatbelt {
    pub(super) fn new(store: WorkingCopyStore) -> Self {
        Self { store }
    }
}

#[cfg(target_os = "macos")]
impl SandboxBackend for Seatbelt {
    fn kind(&self) -> SandboxKind {
        SandboxKind::Seatbelt
    }

    fn prepare(&self, id: &SessionId, cwd: &Path) -> Result<WorkingCopy, Error> {
        if !seatbelt_available() {
            return Err(Error::SandboxUnavailable(SandboxKind::Seatbelt));
        }
        // Working copy: a git worktree when possible, else the user's dir —
        // the write confinement applies either way.
        let mut ws = GitWorktree::new(self.store.clone())
            .prepare(id, cwd)
            .unwrap_or(WorkingCopy {
                kind: SandboxKind::Seatbelt,
                repo: None,
                branch: None,
                path: cwd.to_path_buf(),
            });
        ws.kind = SandboxKind::Seatbelt;
        Ok(ws)
    }

    fn wrap_command(
        &self,
        working_copy: &WorkingCopy,
        cmd: &mut CommandBuilder,
    ) -> Result<(), Error> {
        *cmd.get_argv_mut() = wrap_argv(&working_copy.path, cmd.get_argv());
        Ok(())
    }

    fn teardown(&self, working_copy: &WorkingCopy) -> Result<(), Error> {
        // The sandbox is per-process, so there is nothing to tear down beyond
        // the working copy.
        teardown_worktree(working_copy);
        Ok(())
    }
}

/// `SandboxKind::Seatbelt` stays defined everywhere (it is core data); off
/// macOS the backend refuses honestly instead of silently downgrading.
#[cfg(not(target_os = "macos"))]
#[derive(Debug)]
pub struct Seatbelt;

#[cfg(not(target_os = "macos"))]
impl Seatbelt {
    pub(super) fn new(_store: WorkingCopyStore) -> Self {
        Self
    }
}

#[cfg(not(target_os = "macos"))]
impl SandboxBackend for Seatbelt {
    fn kind(&self) -> SandboxKind {
        SandboxKind::Seatbelt
    }

    fn prepare(&self, _id: &SessionId, _cwd: &Path) -> Result<WorkingCopy, Error> {
        Err(Error::SandboxUnavailable(SandboxKind::Seatbelt))
    }

    fn teardown(&self, _workspace: &WorkingCopy) -> Result<(), Error> {
        Ok(())
    }
}

/// Whether `sandbox-exec` is present and runnable. Cached once per process —
/// this is probed on every `Info` handshake, not just at startup.
#[cfg(target_os = "macos")]
pub(super) fn seatbelt_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        Command::new("sandbox-exec")
            .args(["-p", "(version 1) (allow default)", "/bin/true"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// The profile: allow everything, deny file writes, then re-allow writes to
/// the working copy, `$HOME`, `$TMPDIR`, and `/dev`. Reads, network, and
/// subprocess spawning stay allowed — the harness is a cloud-LLM client and
/// needs them.
///
/// Known first-cut limitations (shared with bubblewrap): `$HOME` is writable
/// in full, so secrets under `$HOME` (`.ssh`, `.aws`, …) are not hidden; and
/// a worktree's `git commit` writes to the *main* repo's `.git` (outside the
/// working copy), so it is denied. Tightening both is future work.
#[cfg(target_os = "macos")]
const PROFILE: &str = "\
(version 1)\n\
(allow default)\n\
(deny file-write*)\n\
(allow file-write* (subpath (param \"WORKING_COPY\")))\n\
(allow file-write* (subpath (param \"HOME\")))\n\
(allow file-write* (subpath (param \"TMPDIR\")))\n\
(allow file-write* (subpath \"/dev\"))\n";

/// Canonicalize a path for the profile: Seatbelt resolves symlinks when it
/// checks a write, so `/tmp` must be expressed as `/private/tmp` to match.
/// Best-effort — a path that does not exist yet is passed through as-is.
#[cfg(target_os = "macos")]
fn real(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(target_os = "macos")]
fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

#[cfg(target_os = "macos")]
fn tmpdir() -> PathBuf {
    std::env::var_os("TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// The `sandbox-exec` argv: `-D` params for the writable paths, `-p` with the
/// profile, then the harness command verbatim.
#[cfg(target_os = "macos")]
fn wrap_argv(working_copy: &Path, argv: &[OsString]) -> Vec<OsString> {
    let mut out: Vec<OsString> = Vec::with_capacity(argv.len() + 8);
    out.push("sandbox-exec".into());
    for (key, val) in [
        ("WORKING_COPY", real(working_copy)),
        ("HOME", real(&home_dir())),
        ("TMPDIR", real(&tmpdir())),
    ] {
        let mut arg = OsString::from(format!("-D{key}="));
        arg.push(val.as_os_str());
        out.push(arg);
    }
    out.push("-p".into());
    out.push(PROFILE.into());
    out.extend_from_slice(argv);
    out
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn wraps_argv_with_sandbox_exec_and_preserves_command() {
        let dir = std::env::temp_dir().join(format!("bao-seatbelt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let argv = ["bash", "-c", "echo hi"]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
        let wrapped = wrap_argv(&dir, &argv);
        assert_eq!(wrapped[0].to_string_lossy(), "sandbox-exec");
        // `-p` is followed by the profile, then the command verbatim.
        let p = wrapped
            .iter()
            .position(|a| a == "-p")
            .expect("profile flag");
        let tail: Vec<String> = wrapped[p + 2..]
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(tail, ["bash", "-c", "echo hi"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn confines_writes_to_the_workspace() {
        if !seatbelt_available() {
            return; // honest skip where sandbox-exec is missing
        }
        let root = std::env::temp_dir().join(format!("bao-seatbelt-{}", std::process::id()));
        let ws = root.join("ws");
        std::fs::create_dir_all(&ws).unwrap();

        // A write inside the working copy is allowed.
        let ok = ws.join("ok.txt");
        let script = format!("echo hi > '{}'", ok.display());
        let argv = ["sh", "-c", script.as_str()]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
        let wrapped = wrap_argv(&ws, &argv);
        let st = Command::new(&wrapped[0])
            .args(&wrapped[1..])
            .status()
            .unwrap();
        assert!(st.success(), "working_copy write must succeed");
        assert!(ok.exists());

        // A write outside the working copy is denied.
        let deny = root.join("deny.txt");
        let script = format!("echo hi > '{}'", deny.display());
        let argv = ["sh", "-c", script.as_str()]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
        let wrapped = wrap_argv(&ws, &argv);
        let st = Command::new(&wrapped[0])
            .args(&wrapped[1..])
            .status()
            .unwrap();
        assert!(!st.success(), "out-of-working_copy write must be denied");
        assert!(!deny.exists());

        std::fs::remove_dir_all(&root).unwrap();
    }
}
