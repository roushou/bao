//! `bao daemon` — run and stop the daemon on this machine.

use std::{
    process,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Result, bail};
use bao_core::types::Status;
use bao_daemon::{pid::PidFile, session::Manager};
use clap::Subcommand;

use super::Context;

/// Run or stop the daemon that hosts your sessions.
#[derive(Subcommand, Debug)]
pub enum DaemonCommand {
    Run,
    Stop,
}

/// Run Bao on this machine: the daemon that hosts your sessions and keeps
/// them alive.
#[derive(clap::Args, Debug)]
pub struct DaemonCmd {
    #[command(subcommand)]
    pub cmd: Option<DaemonCommand>,
}

impl DaemonCmd {
    pub async fn run(&self, ctx: &Context) -> Result<()> {
        if matches!(self.cmd, Some(DaemonCommand::Stop)) {
            return stop(ctx);
        }
        // Holding the PID lock is what makes this *the* daemon: acquire it
        // before touching any other state.
        let _pid = PidFile::acquire(ctx.home.daemon_pid_file())
            .map_err(|e| anyhow::anyhow!("bao daemon: {e}; not starting another"))?;
        let manager = Arc::new(Manager::open(&ctx.home)?);
        let restored = manager.list();
        if !restored.is_empty() {
            eprintln!(
                "bao daemon: restored {} session(s) from disk ({} interrupted)",
                restored.len(),
                restored
                    .iter()
                    .filter(|s| s.status() == Status::Interrupted)
                    .count()
            );
        }
        let (actual, handle) = bao_daemon::serve(ctx.addr.clone(), manager.clone()).await?;
        eprintln!(
            "bao daemon: host {} · listening on {actual} (sessions in {})",
            bao_daemon::hostname::resolve(),
            manager.sessions_dir().display()
        );
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                eprintln!("bao daemon: shutting down, stopping sessions…");
                manager.kill_all();
                manager.flush_all().await;
            }
            _ = handle => {}
        }
        Ok(())
    }
}

/// Stop a locally-running daemon by its PID file — never guessing. The PID
/// is only trusted after `/proc/<pid>/exe` confirms it is actually a bao
/// binary (a recycled PID must not be killed). The PID file itself is left
/// in place; the lock release on daemon death makes staleness detectable
/// and the next `run` overwrites the file.
fn stop(ctx: &Context) -> Result<()> {
    let path = ctx.home.daemon_pid_file();
    let Some(pid) = bao_daemon::pid::read_pid(&path)
        .map_err(|_| anyhow::anyhow!("corrupt PID file ({})", path.display()))?
    else {
        bail!("no daemon recorded here ({})", path.display());
    };

    // The PID file may be stale (e.g. a SIGKILLed daemon); verify liveness

    let exe = std::fs::read_link(format!("/proc/{pid}/exe"))
        .map_err(|_| anyhow::anyhow!("daemon (pid {pid}) is not running"))?;
    if !exe.to_string_lossy().contains("/bao") {
        bail!(
            "PID {pid} is not a bao binary ({}) — refusing to kill it",
            exe.display()
        );
    }

    // `unsafe_code = forbid` rules out libc::kill directly; the kill(1)
    // utility is always present on the platforms Bao's daemon targets.
    let ok = process::Command::new("kill")
        .arg(pid.to_string())
        .status()
        .map_err(|e| anyhow::anyhow!("failed to run `kill`: {e}"))?
        .success();
    if !ok {
        bail!("failed to signal daemon (pid {pid})");
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && std::path::Path::new(&format!("/proc/{pid}")).exists() {
        std::thread::sleep(Duration::from_millis(50));
    }
    println!("stopped daemon (pid {pid})");
    Ok(())
}
