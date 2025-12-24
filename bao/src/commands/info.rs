use std::path::PathBuf;

use baobao_manifest::BaoToml;
use clap::Args;
use eyre::Result;

use super::UnwrapOrExit;
use crate::{
    ops,
    reports::{Report, TerminalOutput},
};

#[derive(Args)]
pub struct InfoCommand {
    /// Path to bao.toml (defaults to ./bao.toml)
    #[arg(short, long, default_value = "bao.toml")]
    pub config: PathBuf,
}

impl InfoCommand {
    pub fn run(&self) -> Result<()> {
        let bao_toml = BaoToml::open(&self.config).unwrap_or_exit();
        let manifest = bao_toml.schema();

        let report = ops::info(manifest, &self.config);
        report.render(&mut TerminalOutput::new());

        Ok(())
    }
}
