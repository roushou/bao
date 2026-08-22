//! `bao list` — list sessions.

use anyhow::Result;
use bao_core::types::TermStrExt;
use clap::Args;

use super::Context;

/// List sessions.
#[derive(Args, Debug)]
pub struct ListCmd;

impl ListCmd {
    pub async fn run(&self, ctx: &Context) -> Result<()> {
        let mut conn = ctx.connect().await?;
        let sessions = conn.list().await?;
        if sessions.is_empty() {
            println!("no sessions");
            return Ok(());
        }
        let header = format!(
            "{:<18} {:<10} {:<24} {:<12} {:<12} {:<16} {:<8} CWD",
            "NAME", "ID", "COMMAND", "STATUS", "ISOLATION", "BRANCH", "AGE"
        );
        println!("{header}");
        for s in &sessions {
            let age = s.age_secs;
            println!(
                "{:<18} {:<10} {:<24} {:<12} {:<12} {:<16} {:<8}s {}",
                s.name.as_deref().unwrap_or("").truncate(17),
                s.id,
                s.command.as_str().truncate(23),
                s.status,
                s.workspace.kind,
                s.workspace.branch.as_deref().unwrap_or("").truncate(15),
                age,
                s.cwd.display(),
            );
        }
        Ok(())
    }
}
