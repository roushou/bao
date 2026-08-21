//! `bao daemon` — run Bao on this machine.

use std::sync::Arc;

use anyhow::Result;
use bao_core::types::Status;
use bao_daemon::session::Manager;
use clap::Args;

use super::Context;

/// Run Bao on this machine: the daemon that hosts your sessions and keeps
/// them alive.
#[derive(Args, Debug)]
pub struct DaemonCmd;

impl DaemonCmd {
    pub async fn run(&self, ctx: &Context) -> Result<()> {
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
            manager.dir.display()
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
