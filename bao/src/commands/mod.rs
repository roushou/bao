mod add;
mod bake;
mod check;
mod clean;
mod completions;
mod fmt;
mod info;
mod init;
mod list;
mod remove;
mod run;

use add::AddCommand;
use bake::BakeCommand;
use check::CheckCommand;
use clap::{Parser, Subcommand};
use clean::CleanCommand;
use completions::CompletionsCommand;
use eyre::Result;
use fmt::FmtCommand;
use info::InfoCommand;
use init::InitCommand;
use list::ListCommand;
use remove::RemoveCommand;
use run::RunCommand;

/// Extension trait for exiting on manifest errors with pretty formatting
pub(crate) trait UnwrapOrExit<T> {
    fn unwrap_or_exit(self) -> T;
}

impl<T> UnwrapOrExit<T> for baobao_manifest::Result<T> {
    fn unwrap_or_exit(self) -> T {
        match self {
            Ok(v) => v,
            Err(e) => {
                eprintln!("{:?}", miette::Report::new(*e));
                std::process::exit(1);
            }
        }
    }
}

#[derive(Parser)]
#[command(name = "bao")]
#[command(version)]
#[command(about = "Generate CLI applications from TOML definitions")]
pub(crate) struct Cli {
    #[command(subcommand)]
    command: Commands,
}

impl Cli {
    pub fn run(&self) -> Result<()> {
        match &self.command {
            Commands::Init(cmd) => cmd.run(),
            Commands::Bake(cmd) => cmd.run(),
            Commands::Check(cmd) => cmd.run(),
            Commands::Clean(cmd) => cmd.run(),
            Commands::Fmt(cmd) => cmd.run(),
            Commands::Info(cmd) => cmd.run(),
            Commands::Add(cmd) => cmd.run(),
            Commands::Remove(cmd) => cmd.run(),
            Commands::List(cmd) => cmd.run(),
            Commands::Completions(cmd) => cmd.run(),
            Commands::Run(cmd) => cmd.run(),
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new CLI project
    Init(InitCommand),

    /// Generate CLI code from bao.toml
    Bake(BakeCommand),

    /// Validate bao.toml without generating code
    Check(CheckCommand),

    /// Remove orphaned generated files
    Clean(CleanCommand),

    /// Format bao.toml
    Fmt(FmtCommand),

    /// Show project information
    Info(InfoCommand),

    /// Add a command or context to bao.toml
    Add(AddCommand),

    /// Remove a command or context from bao.toml
    Remove(RemoveCommand),

    /// List commands and context defined in bao.toml
    List(ListCommand),

    /// Generate shell completions
    Completions(CompletionsCommand),

    /// Run the generated CLI (shortcut for cargo run --)
    Run(RunCommand),
}
