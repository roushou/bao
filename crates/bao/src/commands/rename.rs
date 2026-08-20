//! `bao rename` — rename a session.

use anyhow::Result;
use clap::Args;

use super::Context;

/// Rename a session (empty name clears it).
#[derive(Args, Debug)]
pub struct RenameCmd {
    /// session id or name
    pub session: String,
    /// new name (empty clears the name)
    pub name: String,
}

impl RenameCmd {
    pub async fn run(&self, ctx: &Context) -> Result<()> {
        let name = if self.name.trim().is_empty() {
            None
        } else {
            Some(self.name.clone())
        };
        let mut conn = ctx.connect().await?;
        conn.rename(&ctx.session_id(&self.session), name).await?;
        println!("renamed {} to {}", self.session, self.name.trim());
        Ok(())
    }
}
