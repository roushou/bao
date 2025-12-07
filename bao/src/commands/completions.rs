use std::io;

use clap::{Args, CommandFactory};
use eyre::Result;

use super::Cli;

#[derive(Args)]
pub struct CompletionsCommand {
    /// Shell to generate completions for
    shell: clap_complete::Shell,
}

impl CompletionsCommand {
    pub fn run(&self) -> Result<()> {
        let mut cmd = Cli::command();
        clap_complete::generate(self.shell, &mut cmd, "bao", &mut io::stdout());
        Ok(())
    }
}
