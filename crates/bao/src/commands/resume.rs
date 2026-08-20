//! `bao resume` — relaunch an interrupted session with its conversation.

use std::io::IsTerminal;

use anyhow::Result;
use clap::Args;

use super::Context;

/// Resume an interrupted session (relaunch the session with its conversation
/// and attach).
#[derive(Args, Debug)]
pub struct ResumeCmd {
    /// session id or name
    pub session: String,
}

impl ResumeCmd {
    pub async fn run(&self, ctx: &Context) -> Result<()> {
        if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
            anyhow::bail!("`bao resume` needs a terminal — run it inside a real TTY");
        }
        let mut conn = ctx.connect().await?;
        let meta = conn
            .resume(&ctx.session_id(&self.session), ctx.terminal_size())
            .await?;
        let sid = meta.id.clone();
        eprintln!("bao: resumed session {sid}");
        bao_tui::Tui::run_attached(ctx.addr(), sid).await?;
        Ok(())
    }
}
