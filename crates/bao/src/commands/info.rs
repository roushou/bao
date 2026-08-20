//! `bao info` — the daemon's self-description.

use anyhow::Result;
use clap::Args;

use super::Context;

/// Show this daemon's identity and capabilities.
#[derive(Args, Debug)]
pub struct InfoCmd;

impl InfoCmd {
    pub async fn run(&self, ctx: &Context) -> Result<()> {
        let conn = ctx.connect().await?;
        let info = conn.info().clone();
        println!("host: {}", info.host);
        println!("protocol version: {}", info.protocol_version);
        let backends = info
            .isolation_backends
            .iter()
            .map(|k| k.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        println!("isolation: {backends}");
        Ok(())
    }
}
