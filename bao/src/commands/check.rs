use std::path::PathBuf;

use baobao_manifest::{BaoToml, Command};
use clap::Args;
use eyre::Result;

#[derive(Args)]
pub struct CheckCommand {
    /// Path to bao.toml (defaults to ./bao.toml)
    #[arg(short, long, default_value = "bao.toml")]
    pub config: PathBuf,
}

impl CheckCommand {
    /// Run the check command
    pub fn run(&self) -> Result<()> {
        let bao_toml = match BaoToml::open(&self.config) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("{:?}", miette::Report::new(*e));
                std::process::exit(1);
            }
        };

        let schema = bao_toml.schema();

        println!("✓ {} is valid\n", self.config.display());

        // CLI info
        println!("  {} v{}", schema.cli.name, schema.cli.version);
        if let Some(desc) = &schema.cli.description {
            println!("  {}\n", desc);
        } else {
            println!();
        }

        // Commands
        let cmd_count = Self::count_commands(&schema.commands);
        println!(
            "  {} command{}:",
            cmd_count,
            if cmd_count == 1 { "" } else { "s" }
        );
        Self::print_commands(&schema.commands, "    ");

        // Context
        if !schema.context.is_empty() {
            let ctx_count = schema.context.len();
            println!(
                "\n  {} context field{}:",
                ctx_count,
                if ctx_count == 1 { "" } else { "s" }
            );
            for (name, field) in schema.context.fields() {
                println!("    {} ({})", name, field.rust_type());
            }
        }

        Ok(())
    }

    fn count_commands(commands: &std::collections::HashMap<String, Command>) -> usize {
        let mut total = 0;
        for cmd in commands.values() {
            if cmd.has_subcommands() {
                total += Self::count_commands(&cmd.commands);
            } else {
                total += 1;
            }
        }
        total
    }

    fn print_commands(commands: &std::collections::HashMap<String, Command>, indent: &str) {
        let mut names: Vec<_> = commands.keys().collect();
        names.sort();
        for name in names {
            let cmd = &commands[name];
            if cmd.has_subcommands() {
                println!("{}{}", indent, name);
                Self::print_commands(&cmd.commands, &format!("{}  ", indent));
            } else {
                println!("{}{}", indent, name);
            }
        }
    }
}
