mod commands;
mod language;
mod ops;
mod reports;

use clap::Parser;
use eyre::Result;

use crate::commands::Cli;

fn main() -> Result<()> {
    color_eyre::install()?;

    Cli::parse().run()
}
