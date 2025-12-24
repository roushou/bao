use std::path::PathBuf;

use baobao_manifest::{BaoToml, Language};
use clap::Args;
use eyre::Result;

use super::UnwrapOrExit;
use crate::{
    language::LanguageSupport,
    ops,
    reports::{Report, TerminalOutput},
};

#[derive(Args)]
pub struct BakeCommand {
    /// Path to bao.toml (defaults to ./bao.toml)
    #[arg(short, long, default_value = "bao.toml")]
    pub config: PathBuf,

    /// Output directory (defaults to current directory)
    #[arg(short, long, default_value = ".")]
    pub output: PathBuf,

    /// Preview generated code without writing to disk
    #[arg(long)]
    pub dry_run: bool,

    /// Target language (overrides bao.toml setting)
    #[arg(short, long)]
    pub language: Option<Language>,

    /// Output intermediate representations for debugging
    #[arg(long)]
    pub visualize: bool,
}

impl BakeCommand {
    pub fn run(&self) -> Result<()> {
        let bao_toml = BaoToml::open(&self.config).unwrap_or_exit();
        let manifest = bao_toml.schema();
        let lang = LanguageSupport::get(self.language.unwrap_or(manifest.cli.language));

        let report = ops::bake(
            manifest,
            lang,
            ops::bake::BakeOptions {
                output_dir: &self.output,
                dry_run: self.dry_run,
                visualize: self.visualize,
            },
        )?;

        report.render(&mut TerminalOutput::new());
        Ok(())
    }
}
