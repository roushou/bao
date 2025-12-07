use std::process::Command;

use clap::Args;
use eyre::{Result, WrapErr};

#[derive(Args)]
pub struct RunCommand {
    /// Arguments to pass to the CLI
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

impl RunCommand {
    pub fn run(&self) -> Result<()> {
        let status = Command::new("cargo")
            .arg("run")
            .arg("--")
            .args(&self.args)
            .status()
            .wrap_err("Failed to run cargo")?;

        std::process::exit(status.code().unwrap_or(1));
    }
}
