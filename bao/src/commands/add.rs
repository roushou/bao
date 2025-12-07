use std::path::PathBuf;

use baobao_schema::BaoToml;
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

        // Build the TOML to append
        let toml_section = if args.name.contains('/') {
            // Subcommand: users/create -> [commands.users.commands.create]
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

        let new_content = format!(
            "{}\n\n{}\ndescription = \"{}\"\n",
            bao_toml.content().trim_end(),
            toml_section,
            args.description
        );

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

        let context_toml = match args.context_type.as_str() {
            "sqlite" => format!(
                "\n\n[context.{}]\ntype = \"sqlite\"\nenv = \"DATABASE_URL\"\ncreate_if_missing = true\njournal_mode = \"wal\"\nforeign_keys = true\n",
                field_name
            ),
            "postgres" | "mysql" => format!(
                "\n\n[context.{}]\ntype = \"{}\"\nenv = \"DATABASE_URL\"\n",
                field_name, args.context_type
            ),
            "http" => "\n\n[context.http]\n".to_string(),
            _ => unreachable!(),
        };

        let new_content = format!("{}{}", bao_toml.content().trim_end(), context_toml);
        bao_toml.set_content(new_content)?;
        bao_toml.save()?;
        println!(
            "Added context '{}' (type: {})",
            field_name, args.context_type
        );

        Ok(())
    }
}
