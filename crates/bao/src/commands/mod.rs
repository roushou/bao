//! The `bao` command set.
//!
//! Each command is a struct carrying its clap args, with a `run(&Context)`
//! method — no free functions, no parameter soup. `Cli`/`Cmd` are the clap
//! surface; `Context` is the shared invocation state every command gets.

mod attach;
mod daemon;
mod info;
mod launch;
mod list;
mod profiles;
mod rename;
mod resume;
mod rm;
mod stop;

use std::{
    io::IsTerminal,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use anyhow::Result;
pub use attach::AttachCmd;
use bao_client::Conn;
use bao_core::types::{Command, SessionId, TerminalSize};
use bao_daemon::Home;
use bao_transport::{Addr, DEFAULT_PORT};
use clap::{Parser, Subcommand};
pub use daemon::DaemonCmd;
pub use info::InfoCmd;
pub use launch::LaunchCmd;
pub use list::ListCmd;
pub use profiles::ProfilesCmd;
pub use rename::RenameCmd;
pub use resume::ResumeCmd;
pub use rm::RmCmd;
pub use stop::StopCmd;

#[derive(Parser)]
#[command(
    name = "bao",
    version,
    about = "supervise your AI coding agents",
    after_help = "Run `bao` with no command to open the overview (the daemon starts automatically)."
)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Option<Cmd>,

    /// port the host daemon listens on
    #[arg(long, global = true)]
    pub port: Option<u16>,
}

#[derive(Subcommand)]
pub enum Cmd {
    Daemon(DaemonCmd),
    Launch(LaunchCmd),
    Attach(AttachCmd),
    Resume(ResumeCmd),
    Rename(RenameCmd),
    List(ListCmd),
    Info(InfoCmd),
    Stop(StopCmd),
    Rm(RmCmd),
    Profiles(ProfilesCmd),
}

/// Shared invocation state: resolved home, daemon address, and the loaded
/// harness profiles. Computed once, passed to every command's `run`.
pub struct Context {
    home: Home,
    addr: Addr,
    profiles: ProfileMap,
}

/// Resolve the parsed CLI into invocation state. Borrowed (`&Cli`) because
/// `Cli::run` still moves `self.cmd` after building the context.
impl From<&Cli> for Context {
    fn from(cli: &Cli) -> Self {
        let root = std::env::var_os("BAO_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|h| h.join(".bao")))
            .unwrap_or_else(|| PathBuf::from(".bao"));
        let port = cli
            .port
            .or_else(|| std::env::var("BAO_PORT").ok().and_then(|p| p.parse().ok()))
            .unwrap_or(DEFAULT_PORT);
        let profiles = ProfileMap::load(&root);
        Context {
            addr: Addr::local(port),
            home: Home::new(&root),
            profiles,
        }
    }
}

impl Context {
    /// A connection to the local daemon.
    /// A connection to the local daemon. Returns the typed client error so
    /// callers can branch (e.g. `ensure_daemon` on `Unreachable`).
    pub async fn connect(&self) -> Result<Conn, bao_client::Error> {
        Conn::connect(&self.addr).await
    }

    /// The current terminal size (or a sane default when not a TTY).
    pub fn terminal_size(&self) -> TerminalSize {
        let (cols, rows) = crossterm::terminal::size().unwrap_or((120, 40));
        TerminalSize { cols, rows }
    }

    /// The daemon address this invocation is talking to.
    pub fn addr(&self) -> Addr {
        self.addr.clone()
    }

    /// Connect to the daemon, starting it if it isn't running. The
    /// the interactive surface (bare `bao`) uses this — like tmux,
    /// the entry point brings its own server up.
    pub async fn ensure_daemon(&self) -> Result<()> {
        match self.connect().await {
            Ok(_) => Ok(()),
            Err(bao_client::Error::Unreachable { .. }) => {
                std::fs::create_dir_all(self.home.root())?;
                let daemon_log = self.home.root().join("daemon.log");
                eprintln!("bao: starting daemon (log: {})", daemon_log.display());
                let log = std::fs::File::create(&daemon_log)?;
                std::process::Command::new(std::env::current_exe()?)
                    .arg("daemon")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::from(log))
                    .process_group(0)
                    .spawn()?;
                for _ in 0..20 {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    if self.connect().await.is_ok() {
                        return Ok(());
                    }
                }
                anyhow::bail!("bao daemon did not start — see {}", daemon_log.display());
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Resolve user-typed session input to a [`SessionId`] (empty/unknown
    /// text becomes a default id; the daemon resolves prefixes and names).
    pub fn session_id(&self, s: &str) -> SessionId {
        SessionId::from_str(s).unwrap_or_default()
    }
}

/// Harness profiles: name -> launch command. Built-in default is `pi`; a
/// user-supplied `profiles.json` at the bao home overrides/adds entries.
pub struct ProfileMap {
    entries: Vec<(String, Command)>,
}

impl ProfileMap {
    fn load(home: &Path) -> Self {
        let mut entries = vec![("pi".to_string(), Command::parse("pi").unwrap())];
        if let Ok(raw) = std::fs::read_to_string(home.join("profiles.json")) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(obj) = v.as_object() {
                    for (k, val) in obj {
                        if let Some(cmd) = val.as_str() {
                            if let Ok(command) = Command::parse(cmd) {
                                entries.push((k.clone(), command));
                            }
                        }
                    }
                }
            }
        }
        ProfileMap { entries }
    }

    fn get(&self, name: &str) -> Option<&Command> {
        self.entries.iter().find(|(n, _)| n == name).map(|(_, c)| c)
    }

    fn list(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = self
            .entries
            .iter()
            .map(|(n, c)| (n.clone(), c.display()))
            .collect();
        out.sort();
        out
    }
}

impl Cli {
    /// Resolve the invocation and route it to its command. With no command,
    /// `bao` opens the overview — the primary interface.
    pub async fn run(self) -> Result<()> {
        let ctx = Context::from(&self);
        match self.cmd {
            None => {
                if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
                    anyhow::bail!(
                        "`bao` needs a terminal to open the overview — run it in a real TTY, or use `bao list` / `bao launch --detach`"
                    );
                }
                // The overview brings its own server up (like tmux).
                ctx.ensure_daemon().await?;
                bao_tui::Tui::run(ctx.addr).await?;
                Ok(())
            }
            Some(Cmd::Daemon(c)) => c.run(&ctx).await,
            Some(Cmd::Launch(c)) => c.run(&ctx).await,
            Some(Cmd::Attach(c)) => c.run(&ctx).await,
            Some(Cmd::Resume(c)) => c.run(&ctx).await,
            Some(Cmd::Rename(c)) => c.run(&ctx).await,
            Some(Cmd::List(c)) => c.run(&ctx).await,
            Some(Cmd::Info(c)) => c.run(&ctx).await,
            Some(Cmd::Stop(c)) => c.run(&ctx).await,
            Some(Cmd::Rm(c)) => c.run(&ctx).await,
            Some(Cmd::Profiles(c)) => c.run(&ctx).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[test]
    fn bare_bao_parses_with_no_command() {
        // `bao` with no subcommand is the primary entry point — it opens
        // the overview.
        let cli = Cli::try_parse_from(["bao"]).unwrap();
        assert!(cli.cmd.is_none());
        assert_eq!(cli.port, None);
    }

    #[test]
    fn subcommands_still_parse() {
        let cli = Cli::try_parse_from(["bao", "list"]).unwrap();
        assert!(matches!(cli.cmd, Some(Cmd::List(_))));
        let cli = Cli::try_parse_from(["bao", "daemon"]).unwrap();
        assert!(matches!(cli.cmd, Some(Cmd::Daemon(_))));
    }
}
