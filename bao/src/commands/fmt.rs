use std::path::PathBuf;

use baobao_manifest::BaoToml;
use clap::Args;
use eyre::Result;

use super::UnwrapOrExit;

#[derive(Args)]
pub struct FmtCommand {
    /// Path to bao.toml (defaults to ./bao.toml)
    #[arg(short, long, default_value = "bao.toml")]
    pub config: PathBuf,

    /// Check if file is formatted without making changes (exit 1 if not)
    #[arg(long)]
    pub check: bool,
}

impl FmtCommand {
    pub fn run(&self) -> Result<()> {
        let bao_toml = BaoToml::open(&self.config).unwrap_or_exit();
        let formatted = bao_toml.to_formatted_string();

        if self.check {
            if bao_toml.content() != formatted {
                eprintln!("error: {} is not formatted", self.config.display());
                eprintln!("Run `bao fmt` to fix.");
                std::process::exit(1);
            }
            println!("{} is formatted", self.config.display());
        } else if bao_toml.content() == formatted {
            println!("{} is already formatted", self.config.display());
        } else {
            std::fs::write(&self.config, &formatted)?;
            println!("Formatted {}", self.config.display());
        }

        Ok(())
    }
}
