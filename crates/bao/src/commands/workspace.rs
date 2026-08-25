//! `bao workspace` — manage workspaces: the named targets sessions are launched at.

use std::path::PathBuf;

use anyhow::Result;
use bao_core::registry::RegistryEntry;
use clap::{Args, Subcommand};

use super::Context;

/// Manage workspaces: named roots sessions are aimed at (`bao workspace add`).
#[derive(Args, Debug)]
pub struct WorkspaceCmd {
    #[command(subcommand)]
    pub cmd: WorkspaceSubCmd,
}

#[derive(Subcommand, Debug)]
pub enum WorkspaceSubCmd {
    /// Register a workspace: alias → root path (defaults to the current
    /// directory).
    Add(WorkspaceAdd),
    /// Forget a workspace by alias. Sessions already launched against it
    /// are untouched.
    Rm { alias: String },
    /// List the workspaces registered on this host.
    List,
}

#[derive(Args, Debug)]
pub struct WorkspaceAdd {
    /// The handle you'll launch with (`bao launch <alias>`)
    pub alias: String,
    /// The workspace root (default: current directory)
    pub path: Option<PathBuf>,
}

impl WorkspaceCmd {
    pub async fn run(&self, ctx: &Context) -> Result<()> {
        ctx.ensure_daemon().await?;
        let mut conn = ctx.connect().await?;
        match &self.cmd {
            WorkspaceSubCmd::Add(add) => {
                let path = add
                    .path
                    .clone()
                    .or_else(|| std::env::current_dir().ok())
                    .ok_or_else(|| {
                        anyhow::anyhow!("no directory given and no current directory")
                    })?;
                conn.registry_put(RegistryEntry::workspace(&add.alias, path)?)
                    .await?;
                println!("registered '{}' — `bao launch {}`", add.alias, add.alias);
            }
            WorkspaceSubCmd::Rm { alias } => {
                let entries = conn.registry_list().await?;
                if let Some(e) = entries.iter().find(|e| e.alias == *alias)
                    && !e.is_workspace()
                {
                    anyhow::bail!("'{alias}' is a profile, not a workspace — see `bao profile rm`");
                }
                conn.registry_remove(alias).await?;
                println!("forgot '{alias}'");
            }
            WorkspaceSubCmd::List => {
                let workspaces: Vec<_> = conn
                    .registry_list()
                    .await?
                    .into_iter()
                    .filter(|e| e.is_workspace())
                    .collect();
                if workspaces.is_empty() {
                    println!(
                        "no workspaces registered on this host — `bao workspace add <alias> [path]`"
                    );
                } else {
                    let width = workspaces.iter().map(|w| w.alias.len()).max().unwrap_or(0);
                    println!("{:<width$}  ROOT", "ALIAS", width = width);
                    for w in workspaces {
                        println!(
                            "{:<width$}  {}",
                            w.alias,
                            w.root()
                                .map(|p| p.display().to_string())
                                .unwrap_or_default(),
                            width = width
                        );
                    }
                }
            }
        }
        Ok(())
    }
}
