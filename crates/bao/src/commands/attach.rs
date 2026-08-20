//! `bao attach` — attach another client to a running session.

use anyhow::Result;
use clap::Args;

use super::Context;

/// Attach a second client to a running session.
#[derive(Args, Debug)]
pub struct AttachCmd {
    /// session id or name
    pub session: String,
}

impl AttachCmd {
    pub async fn run(&self, ctx: &Context) -> Result<()> {
        bao_tui::Tui::run_attached(ctx.addr(), ctx.session_id(&self.session)).await?;
        Ok(())
    }
}
