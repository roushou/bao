use std::path::PathBuf;

use baobao_manifest::{BaoToml, append_section, command_section_header, context_section_header};
use clap::{Args, Subcommand};
use eyre::{Result, bail};

#[derive(Args)]
pub struct AddCommand {
    #[command(subcommand)]
    command: AddSubcommand,
}

#[derive(Subcommand)]
enum AddSubcommand {
    /// Add a new command to bao.toml
    Command(AddCommandArgs),

    /// Add a context field to bao.toml
    Context(AddContextArgs),
}

#[derive(Args)]
struct AddCommandArgs {
    /// Command name (use / for subcommands, e.g., "users/create")
    name: String,

    /// Command description
    #[arg(short, long, default_value = "TODO: add description")]
    description: String,

    /// Path to bao.toml
    #[arg(short, long, default_value = "bao.toml")]
    config: PathBuf,
}

#[derive(Args)]
struct AddContextArgs {
    /// Context type: sqlite, postgres, mysql, or http
    #[arg(name = "type")]
    context_type: String,

    /// Field name (defaults to "database" for db types, "http" for http)
    #[arg(short, long)]
    name: Option<String>,

    /// Path to bao.toml
    #[arg(short, long, default_value = "bao.toml")]
    config: PathBuf,
}

impl AddCommand {
    pub fn run(&self) -> Result<()> {
        match &self.command {
            AddSubcommand::Command(args) => Self::add_command(args),
            AddSubcommand::Context(args) => Self::add_context(args),
        }
    }

    fn add_command(args: &AddCommandArgs) -> Result<()> {
        let mut bao_toml = BaoToml::open(&args.config)?;

        if bao_toml.schema().has_command(&args.name) {
            bail!("Command '{}' already exists", args.name);
        }

        let section = format!(
            "{}\ndescription = \"{}\"",
            command_section_header(&args.name),
            args.description
        );
        let new_content = append_section(bao_toml.content(), &section);

        bao_toml.set_content(new_content)?;
        bao_toml.save()?;
        println!("Added command '{}'", args.name);

        Ok(())
    }

    fn add_context(args: &AddContextArgs) -> Result<()> {
        let valid_types = ["sqlite", "postgres", "mysql", "http"];
        if !valid_types.contains(&args.context_type.as_str()) {
            bail!(
                "Invalid context type '{}'. Valid types: {}",
                args.context_type,
                valid_types.join(", ")
            );
        }

        // HTTP context must use [context.http] - no custom names allowed
        if args.context_type == "http" && args.name.is_some() {
            bail!("HTTP context must be named 'http' (--name is not allowed)");
        }

        let mut bao_toml = BaoToml::open(&args.config)?;

        let field_name = args
            .name
            .clone()
            .unwrap_or_else(|| match args.context_type.as_str() {
                "http" => "http".to_string(),
                _ => "database".to_string(),
            });

        if bao_toml.schema().context.has_field(&field_name) {
            bail!("Context field '{}' already exists", field_name);
        }

        let section = match args.context_type.as_str() {
            "sqlite" => format!(
                "{}\ntype = \"sqlite\"\nenv = \"DATABASE_URL\"\ncreate_if_missing = true\njournal_mode = \"wal\"\nforeign_keys = true",
                context_section_header(&field_name)
            ),
            "postgres" | "mysql" => format!(
                "{}\ntype = \"{}\"\nenv = \"DATABASE_URL\"",
                context_section_header(&field_name),
                args.context_type
            ),
            "http" => context_section_header("http"),
            _ => unreachable!(),
        };

        let new_content = append_section(bao_toml.content(), &section);
        bao_toml.set_content(new_content)?;
        bao_toml.save()?;
        println!(
            "Added context '{}' (type: {})",
            field_name, args.context_type
        );

        Ok(())
    }
}
