use std::path::PathBuf;

use baobao_codegen::schema::{CommandTree, CommandTreeExt, DisplayStyle};
use baobao_manifest::BaoToml;
use clap::Args;
use eyre::Result;

use super::UnwrapOrExit;

#[derive(Args)]
pub struct CheckCommand {
    /// Path to bao.toml (defaults to ./bao.toml)
    #[arg(short, long, default_value = "bao.toml")]
    pub config: PathBuf,
}

impl CheckCommand {
    /// Run the check command
    pub fn run(&self) -> Result<()> {
        let bao_toml = BaoToml::open(&self.config).unwrap_or_exit();
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
        let tree = CommandTree::new(schema);
        let cmd_count = tree.leaf_count();
        println!(
            "  {} command{}:",
            cmd_count,
            if cmd_count == 1 { "" } else { "s" }
        );
        println!(
            "{}",
            tree.display_style(DisplayStyle::Simple).indent("    ")
        );

        // Context
        if !schema.context.is_empty() {
            let ctx_count = schema.context.len();
            println!(
                "\n  {} context field{}:",
                ctx_count,
                if ctx_count == 1 { "" } else { "s" }
            );
            for (name, field) in schema.context.fields() {
                println!("    {} ({})", name, field.type_name());
            }
        }

        Ok(())
    }
}
