//! `bao rm` — forget a session and delete its files.

use anyhow::Result;
use clap::Args;

use super::Context;

/// Forget a session (stop it if running) and delete its files.
#[derive(Args, Debug)]
pub struct RmCmd {
    /// session id or name
    pub session: String,
}

impl RmCmd {
    pub async fn run(&self, ctx: &Context) -> Result<()> {
        let mut conn = ctx.connect().await?;
        conn.rm(&ctx.session_id(&self.session)).await?;
        println!("removed {}", self.session);
        Ok(())
    }
}
