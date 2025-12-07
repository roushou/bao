use std::path::PathBuf;

use baobao_schema::BaoToml;
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

        let section_header = if args.name.contains('/') {
            let parts: Vec<&str> = args.name.split('/').collect();
            let mut path = String::from("[commands");
            for part in &parts[..parts.len() - 1] {
                path.push('.');
                path.push_str(part);
                path.push_str(".commands");
            }
            path.push('.');
            path.push_str(parts.last().unwrap());
            path.push(']');
            path
        } else {
            format!("[commands.{}]", args.name)
        };

        let new_content = Self::remove_toml_section(bao_toml.content(), &section_header);
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

        let section_header = format!("[context.{}]", args.name);
        let new_content = Self::remove_toml_section(bao_toml.content(), &section_header);
        bao_toml.set_content(new_content)?;
        bao_toml.save()?;
        println!("Removed context '{}'", args.name);

        Ok(())
    }

    fn remove_toml_section(content: &str, section_header: &str) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let mut result = Vec::new();
        let mut skip = false;
        let mut skip_blank_after = false;

        for line in lines {
            if line.trim() == section_header {
                skip = true;
                skip_blank_after = true;
                continue;
            }

            if skip {
                // Stop skipping when we hit another section
                if line.starts_with('[') {
                    skip = false;
                    skip_blank_after = false;
                } else {
                    continue;
                }
            }

            // Skip blank lines immediately after removed section
            if skip_blank_after && line.trim().is_empty() {
                skip_blank_after = false;
                continue;
            }

            result.push(line);
        }

        // Clean up trailing blank lines
        while result.last().is_some_and(|l| l.trim().is_empty()) {
            result.pop();
        }

        if result.is_empty() {
            String::new()
        } else {
            format!("{}\n", result.join("\n"))
        }
    }
}
