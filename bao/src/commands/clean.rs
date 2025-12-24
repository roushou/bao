use std::path::PathBuf;

use baobao_manifest::BaoToml;
use clap::Args;
use eyre::Result;

use super::UnwrapOrExit;
use crate::{
    language::LanguageSupport,
    ops,
    reports::{Report, TerminalOutput},
};

#[derive(Args)]
pub struct CleanCommand {
    /// Path to bao.toml (defaults to ./bao.toml)
    #[arg(short, long, default_value = "bao.toml")]
    pub config: PathBuf,

    /// Output directory (defaults to current directory)
    #[arg(short, long, default_value = ".")]
    pub output: PathBuf,

    /// Preview what would be deleted without actually deleting
    #[arg(long)]
    pub dry_run: bool,
}

impl CleanCommand {
    pub fn run(&self) -> Result<()> {
        let bao_toml = BaoToml::open(&self.config).unwrap_or_exit();
        let manifest = bao_toml.schema();
        let lang = LanguageSupport::get(manifest.cli.language);

        let report = ops::clean(
            manifest,
            lang,
            ops::clean::CleanOptions {
                output_dir: &self.output,
                dry_run: self.dry_run,
            },
        )?;

        report.render(&mut TerminalOutput::new());
        Ok(())
    }
}
