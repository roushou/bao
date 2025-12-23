use std::path::PathBuf;

use baobao_codegen::schema::{CommandTree, DisplayStyle};
use baobao_manifest::BaoToml;
use clap::Args;
use eyre::Result;

use super::UnwrapOrExit;

#[derive(Args)]
pub struct ListCommand {
    /// Path to bao.toml (defaults to ./bao.toml)
    #[arg(short, long, default_value = "bao.toml")]
    pub config: PathBuf,
}

impl ListCommand {
    pub fn run(&self) -> Result<()> {
        let bao_toml = BaoToml::open(&self.config).unwrap_or_exit();
        let schema = bao_toml.schema();

        if schema.commands.is_empty() {
            println!("No commands defined");
        } else {
            println!("Commands:");
            let tree = CommandTree::new(schema);
            println!(
                "{}",
                tree.display_style(DisplayStyle::WithDescriptions)
                    .indent("  ")
            );
        }

        if !schema.context.is_empty() {
            println!("\nContext:");
            for (name, field) in schema.context.fields() {
                println!("  {} ({})", name, field.type_name());
            }
        }

        Ok(())
    }
}
