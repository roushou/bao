use std::path::PathBuf;

use baobao_schema::{BaoToml, Command};
use clap::Args;
use eyre::Result;

#[derive(Args)]
pub struct ListCommand {
    /// Path to bao.toml (defaults to ./bao.toml)
    #[arg(short, long, default_value = "bao.toml")]
    pub config: PathBuf,
}

impl ListCommand {
    pub fn run(&self) -> Result<()> {
        let bao_toml = match BaoToml::open(&self.config) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("{:?}", miette::Report::new(*e));
                std::process::exit(1);
            }
        };

        let schema = bao_toml.schema();

        if schema.commands.is_empty() {
            println!("No commands defined");
        } else {
            println!("Commands:");
            let mut names: Vec<_> = schema.commands.keys().collect();
            names.sort();
            for name in names {
                let cmd = &schema.commands[name];
                print_command(name, cmd, 1);
            }
        }

        if !schema.context.is_empty() {
            println!("\nContext:");
            for (name, field) in schema.context.fields() {
                println!("  {} ({})", name, field.rust_type());
            }
        }

        Ok(())
    }
}

fn print_command(name: &str, cmd: &Command, depth: usize) {
    let indent = "  ".repeat(depth);
    println!("{}{} - {}", indent, name, cmd.description);

    if cmd.has_subcommands() {
        let mut subnames: Vec<_> = cmd.commands.keys().collect();
        subnames.sort();
        for subname in subnames {
            let subcmd = &cmd.commands[subname];
            print_command(subname, subcmd, depth + 1);
        }
    }
}
