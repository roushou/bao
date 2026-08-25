//! `bao launch` — launch an session in a session and attach to it.

use std::{io::IsTerminal, path::PathBuf};

use anyhow::Result;
use bao_core::{
    sandbox::{SandboxKind, SandboxSpec},
    types::Command,
};
use clap::Args;

use super::Context;

/// Launch an session in a session and attach to it.
#[derive(Args, Debug)]
pub struct LaunchCmd {
    /// named profile to run (see profiles.json / `bao profiles`)
    #[arg(long)]
    pub profile: Option<String>,
    /// command to run (overrides --profile; default: $BAO_HARNESS_COMMAND or "pi")
    #[arg(long)]
    pub cmd: Option<String>,
    /// working directory for the session (default: current dir; ignored
    /// when WORKSPACE is given — targeting wins)
    #[arg(long)]
    pub dir: Option<String>,
    /// launch into this registered workspace (`bao workspace list`) — wherever
    /// you are
    pub workspace: Option<String>,
    /// human name for the session (default: none)
    #[arg(long)]
    pub name: Option<String>,
    /// requested isolation: inplace | worktree | seatbelt | bubblewrap
    #[arg(long, default_value = "worktree")]
    pub isolation: SandboxKind,
    /// launch without attaching — the session keeps running in the background
    #[arg(long)]
    pub detach: bool,
}

impl LaunchCmd {
    pub async fn run(&self, ctx: &Context) -> Result<()> {
        if !self.detach && (!std::io::stdout().is_terminal() || !std::io::stdin().is_terminal()) {
            anyhow::bail!(
                "`bao launch` needs a terminal — run it inside a real TTY (or use --detach)"
            );
        }
        let command = match &self.profile {
            Some(h) => ctx
                .profiles
                .get(h)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("unknown profile '{h}' — see `bao profiles`"))?,
            None => match &self.cmd {
                Some(c) => Command::parse(c)?,
                None => match std::env::var("BAO_HARNESS_COMMAND") {
                    Ok(c) => Command::parse(&c)?,
                    Err(_) => Command::parse("pi")?,
                },
            },
        };
        let dir = self
            .dir
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok());
        let mut conn = ctx.connect().await?;

        // Targeting: an explicit WORKSPACE aims the session wherever you are.
        // With no argument, an interactive launch asks which registered
        // workspace to aim at — creating a session "just to create it" is
        // useless; Enter falls back to the current directory. Non-interactive
        // launches keep the cwd default so scripts stay scriptable.
        let target = match &self.workspace {
            Some(ws) => Some(ws.clone()),
            None if !self.detach && std::io::stdin().is_terminal() => {
                let workspaces = conn.workspace_list().await?;
                match workspaces.as_slice() {
                    [] => None,
                    listed => {
                        println!("aim at:");
                        for (i, w) in listed.iter().enumerate() {
                            println!("  {}  {}  ({})", i + 1, w.alias, w.root.display());
                        }
                        print!("workspace [1-{}, or Enter for current dir]: ", listed.len());
                        use std::io::Write as _;
                        std::io::stdout().flush()?;
                        let mut line = String::new();
                        std::io::stdin().read_line(&mut line)?;
                        let pick: Option<String> =
                            line.trim().parse::<usize>().ok().and_then(|n| {
                                listed.get(n.checked_sub(1)?).map(|w| w.alias.clone())
                            });
                        if line.trim().is_empty() {
                            None
                        } else if let Some(alias) = pick {
                            Some(alias)
                        } else {
                            anyhow::bail!("no such workspace — see `bao workspace list`");
                        }
                    }
                }
            }
            None => None,
        };

        let meta = match &target {
            Some(ws) => {
                eprintln!("bao: launching `{command}` in workspace {ws}");
                conn.launch_in(
                    ws,
                    Some(command),
                    self.name.clone(),
                    ctx.terminal_size(),
                    SandboxSpec {
                        isolation: self.isolation,
                    },
                )
                .await?
            }
            None => {
                eprintln!(
                    "bao: launching `{command}` in {}",
                    dir.as_ref()
                        .map(|d| d.display().to_string())
                        .unwrap_or_default()
                );
                conn.launch(
                    Some(command),
                    dir,
                    self.name.clone(),
                    ctx.terminal_size(),
                    SandboxSpec {
                        isolation: self.isolation,
                    },
                )
                .await?
            }
        };
        let sid = meta.id.clone();
        if self.detach {
            println!("{sid}");
            return Ok(());
        }
        eprintln!(
            "bao: session {sid} — interact now, or press ^Q to detach and attach again from anywhere"
        );
        bao_tui::Tui::run_attached(ctx.addr(), sid).await?;
        Ok(())
    }
}
