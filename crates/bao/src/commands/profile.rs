//! `bao profile` — manage launch presets: named argv the session runs.

use std::process::exit;

use anyhow::Result;
use bao_core::{registry::RegistryEntry, types::Command};
use clap::{Args, Subcommand};

use super::Context;

/// Manage launch presets (`--profile`).
#[derive(Args, Debug)]
pub struct ProfileCmd {
    #[command(subcommand)]
    pub cmd: ProfileSubCmd,
}

#[derive(Subcommand, Debug)]
pub enum ProfileSubCmd {
    /// Register or replace a preset: name → command.
    Set {
        name: String,
        /// The full command (argv), e.g. `pi --model fast`
        command: Vec<String>,
    },
    /// Forget a preset by name. Sessions already launched are untouched.
    Rm { name: String },
    /// List the presets registered on this host.
    List,
}

impl ProfileCmd {
    pub async fn run(&self, ctx: &Context) -> Result<()> {
        ctx.ensure_daemon().await?;
        let mut conn = ctx.connect().await?;
        match &self.cmd {
            ProfileSubCmd::Set { name, command } => {
                if command.is_empty() {
                    eprintln!("usage: bao profile set <name> <command...>");
                    exit(2);
                }
                // Canonicalize through Command::parse so quoting collapses:
                // `set review "pi --model fast"` stores three argv elements.
                let command = Command::parse(&command.join(" "))?;
                conn.registry_put(RegistryEntry::profile(name, command.as_args().to_vec())?)
                    .await?;
                println!(
                    "registered '{}' ({}) — `bao launch --profile {name}`",
                    name,
                    command.display()
                );
            }
            ProfileSubCmd::Rm { name } => {
                let entries = conn.registry_list().await?;
                if let Some(e) = entries.iter().find(|e| e.alias == *name) {
                    if !e.is_profile() {
                        anyhow::bail!(
                            "'{name}' is a workspace, not a profile — see `bao workspace rm`"
                        );
                    }
                }
                conn.registry_remove(name).await?;
                println!("forgot '{name}'");
            }
            ProfileSubCmd::List => {
                let profiles: Vec<_> = conn
                    .registry_list()
                    .await?
                    .into_iter()
                    .filter(|e| e.is_profile())
                    .collect();
                if profiles.is_empty() {
                    println!(
                        "no profiles registered on this host — `bao profile set <name> <command...>`"
                    );
                } else {
                    let width = profiles.iter().map(|p| p.alias.len()).max().unwrap_or(0);
                    println!("{:<width$}  COMMAND", "NAME", width = width);
                    for p in profiles {
                        println!(
                            "{:<width$}  {}",
                            p.alias,
                            p.argv().map(|a| a.join(" ")).unwrap_or_default(),
                            width = width
                        );
                    }
                }
            }
        }
        Ok(())
    }
}
