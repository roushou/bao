use std::path::PathBuf;

use baobao_codegen::{LanguageCodegen, leaf_commands};
use baobao_codegen_rust::Generator;
use baobao_manifest::{BaoToml, Command, Schema};
use clap::Args;
use eyre::{Context, Result};

#[derive(Args)]
pub struct GenerateCommand {
    /// Path to bao.toml (defaults to ./bao.toml)
    #[arg(short, long, default_value = "bao.toml")]
    pub config: PathBuf,

    /// Output directory (defaults to current directory)
    #[arg(short, long, default_value = ".")]
    pub output: PathBuf,

    /// Preview generated code without writing to disk
    #[arg(long)]
    pub dry_run: bool,
}

impl GenerateCommand {
    /// Run the generate command
    pub fn run(&self) -> Result<()> {
        let bao_toml = match BaoToml::open(&self.config) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("{:?}", miette::Report::new(*e));
                std::process::exit(1);
            }
        };

        let schema = bao_toml.schema();
        let generator = Generator::new(schema);

        if self.dry_run {
            self.run_preview(&generator)
        } else {
            self.run_generation(&generator, schema)
        }
    }

    fn run_generation(&self, generator: &Generator, schema: &Schema) -> Result<()> {
        let result = generator
            .generate(&self.output)
            .wrap_err("Failed to generate code")?;

        // Print header
        println!("{} v{}", schema.cli.name, schema.cli.version);
        if let Some(desc) = &schema.cli.description {
            println!("{}", desc);
        }
        println!();

        // Print commands
        let total = Self::count_commands(schema);
        println!("Commands ({}):", total);
        Self::print_commands(&schema.commands, "  ");
        println!();

        // Print generation summary
        println!("Generated: {}/src/generated/", self.output.display());

        // Report created handler stubs
        if !result.created_handlers.is_empty() {
            println!();
            println!("New handlers:");
            for handler in &result.created_handlers {
                println!("  + src/handlers/{}", handler);
            }
        }

        // Warn about orphan handlers
        if !result.orphan_handlers.is_empty() {
            println!();
            println!("Unused handlers:");
            for orphan in &result.orphan_handlers {
                println!("  - src/handlers/{}.rs", orphan);
            }
        }

        Ok(())
    }

    fn run_preview(&self, generator: &Generator) -> Result<()> {
        let files = generator.preview();

        for file in &files {
            println!("── {} ──", file.path);
            println!("{}", file.content);
        }

        println!("── Summary ──");
        println!("{} files would be generated", files.len());

        Ok(())
    }

    fn count_commands(schema: &Schema) -> usize {
        leaf_commands(schema).len()
    }

    fn print_commands(commands: &std::collections::HashMap<String, Command>, indent: &str) {
        let mut sorted: Vec<_> = commands.iter().collect();
        sorted.sort_by_key(|(name, _)| *name);

        for (name, cmd) in sorted {
            if cmd.has_subcommands() {
                println!("{}{}", indent, name);
                Self::print_commands(&cmd.commands, &format!("{}  ", indent));
            } else {
                let signature = Self::format_command_signature(name, cmd);
                println!("{}{}", indent, signature);
            }
        }
    }

    fn format_command_signature(name: &str, cmd: &Command) -> String {
        let mut parts = vec![name.to_string()];

        // Add args
        let mut sorted_args: Vec<_> = cmd.args.iter().collect();
        sorted_args.sort_by_key(|(n, _)| *n);
        for (arg_name, arg) in sorted_args {
            if arg.required {
                parts.push(format!("<{}>", arg_name));
            } else {
                parts.push(format!("[{}]", arg_name));
            }
        }

        // Add flags
        let mut sorted_flags: Vec<_> = cmd.flags.iter().collect();
        sorted_flags.sort_by_key(|(n, _)| *n);
        let flags: Vec<String> = sorted_flags
            .iter()
            .map(|(flag_name, flag)| {
                if let Some(short) = flag.short_char() {
                    format!("-{}/--{}", short, flag_name)
                } else {
                    format!("--{}", flag_name)
                }
            })
            .collect();

        if !flags.is_empty() {
            parts.push(format!("[{}]", flags.join(" ")));
        }

        parts.join(" ")
    }
}
