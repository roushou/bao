mod add;
mod check;
mod completions;
mod generate;
mod init;
mod list;
mod remove;
mod run;

use add::AddCommand;
use check::CheckCommand;
use clap::{Parser, Subcommand};
use completions::CompletionsCommand;
use eyre::Result;
use generate::GenerateCommand;
use init::InitCommand;
use list::ListCommand;
use remove::RemoveCommand;
use run::RunCommand;

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
            Commands::Generate(cmd) => cmd.run(),
            Commands::Check(cmd) => cmd.run(),
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
    Generate(GenerateCommand),

    /// Validate bao.toml without generating code
    Check(CheckCommand),

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
