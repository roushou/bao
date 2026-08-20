//! `bao stop` — stop a session's session.

use anyhow::Result;
use clap::Args;

use super::Context;

/// Stop a session's session.
#[derive(Args, Debug)]
pub struct StopCmd {
    /// session id or name
    pub session: String,
}

impl StopCmd {
    pub async fn run(&self, ctx: &Context) -> Result<()> {
        let mut conn = ctx.connect().await?;
        conn.stop(&ctx.session_id(&self.session)).await?;
        println!("stopped {}", self.session);
        Ok(())
    }
}
