use std::path::PathBuf;

use baobao_manifest::{
    BaoToml, command_section_header, context_section_header, remove_toml_section,
};
use clap::{Args, Subcommand};
use eyre::{Result, bail};

#[derive(Args)]
pub struct RemoveCommand {
    #[command(subcommand)]
    command: RemoveSubcommand,
}

#[derive(Subcommand)]
enum RemoveSubcommand {
    /// Remove a command from bao.toml
    Command(RemoveCommandArgs),

    /// Remove a context field from bao.toml
    Context(RemoveContextArgs),
}

#[derive(Args)]
struct RemoveCommandArgs {
    /// Command name (use / for subcommands, e.g., "users/create")
    name: String,

    /// Path to bao.toml
    #[arg(short, long, default_value = "bao.toml")]
    config: PathBuf,
}

#[derive(Args)]
struct RemoveContextArgs {
    /// Context field name to remove
    name: String,

    /// Path to bao.toml
    #[arg(short, long, default_value = "bao.toml")]
    config: PathBuf,
}

impl RemoveCommand {
    pub fn run(&self) -> Result<()> {
        match &self.command {
            RemoveSubcommand::Command(args) => Self::remove_command(args),
            RemoveSubcommand::Context(args) => Self::remove_context(args),
        }
    }

    fn remove_command(args: &RemoveCommandArgs) -> Result<()> {
        let mut bao_toml = BaoToml::open(&args.config)?;

        if !bao_toml.schema().has_command(&args.name) {
            bail!("Command '{}' does not exist", args.name);
        }

        let new_content =
            remove_toml_section(bao_toml.content(), &command_section_header(&args.name));
        bao_toml.set_content(new_content)?;
        bao_toml.save()?;
        println!("Removed command '{}'", args.name);

        Ok(())
    }

    fn remove_context(args: &RemoveContextArgs) -> Result<()> {
        let mut bao_toml = BaoToml::open(&args.config)?;

        if !bao_toml.schema().context.has_field(&args.name) {
            bail!("Context field '{}' does not exist", args.name);
        }

        let new_content =
            remove_toml_section(bao_toml.content(), &context_section_header(&args.name));
        bao_toml.set_content(new_content)?;
        bao_toml.save()?;
        println!("Removed context '{}'", args.name);

        Ok(())
    }
}
