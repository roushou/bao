//! `bao profiles` — list known harness profiles.

use anyhow::Result;
use clap::Args;

use super::Context;

/// List known harness profiles.
#[derive(Args, Debug)]
pub struct ProfilesCmd;

impl ProfilesCmd {
    pub async fn run(&self, ctx: &Context) -> Result<()> {
        println!("{:<12} COMMAND", "NAME");
        for (name, cmd) in ctx.profiles.list() {
            println!("{name:<12} {cmd}");
        }
        Ok(())
    }
}
