use std::path::{Path, PathBuf};

use baobao_codegen::LanguageCodegen;
use baobao_codegen_rust::{
    Generator,
    files::{BaoToml, CargoToml, GitIgnore, MainRs},
};
use baobao_core::{File, GeneratedFile};
use clap::Args;
use eyre::{Context, Result};
use miette::Report;

#[derive(Args)]
pub struct InitCommand {
    /// Project name
    pub name: String,

    /// Output directory (defaults to ./<name>)
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

impl InitCommand {
    pub fn run(&self) -> Result<()> {
        let (project_name, output_dir) = Self::resolve_paths(&self.name, self.output.clone())?;
        Self::create_project(&project_name, &output_dir)
    }

    fn resolve_paths(name: &str, output: Option<PathBuf>) -> Result<(String, PathBuf)> {
        if name == "." {
            let cwd = std::env::current_dir().wrap_err("Failed to get current directory")?;
            let dir_name = cwd
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| eyre::eyre!("Current directory has no valid name"))?
                .to_string();
            let output_dir = output.unwrap_or_else(|| PathBuf::from("."));
            Ok((dir_name, output_dir))
        } else {
            let output_dir = output.unwrap_or_else(|| PathBuf::from(name));
            Ok((name.to_string(), output_dir))
        }
    }

    fn create_project(name: &str, output_dir: &Path) -> Result<()> {
        // Create bao.toml
        BaoToml::new(name).write(output_dir)?;

        // Create Cargo.toml
        CargoToml::new(name)
            .with_dependencies(vec![
                ("eyre".to_string(), "0.6".to_string()),
                (
                    "clap".to_string(),
                    r#"{ version = "4", features = ["derive"] }"#.to_string(),
                ),
            ])
            .write(output_dir)?;

        // Create .gitignore
        GitIgnore.write(output_dir)?;

        // Create main.rs (not async for basic init)
        MainRs::new(false).write(output_dir)?;

        // Create handlers/hello.rs with a working example
        // (generator will skip this since file exists, and create handlers/mod.rs)
        File::new(
            output_dir.join("src").join("handlers").join("hello.rs"),
            r#"use crate::context::Context;
use crate::generated::commands::HelloArgs;

pub fn run(_ctx: &Context, args: HelloArgs) -> eyre::Result<()> {
    let name = args.name.unwrap_or_else(|| "World".to_string());
    let greeting = format!("Hello, {}!", name);

    if args.uppercase {
        println!("{}", greeting.to_uppercase());
    } else {
        println!("{}", greeting);
    }

    Ok(())
}
"#,
        )
        .write()?;

        // Generate code from bao.toml
        let bao_toml_path = output_dir.join("bao.toml");
        let schema = match baobao_schema::parse_file(&bao_toml_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{:?}", Report::new(*e));
                std::process::exit(1);
            }
        };

        let generator = Generator::new(&schema);
        let _ = generator
            .generate(output_dir)
            .wrap_err("Failed to generate code")?;

        println!("Created new CLI project in {}", output_dir.display());
        println!();
        println!("Next steps:");
        if output_dir != Path::new(".") {
            println!("  cd {}", output_dir.display());
        }
        println!("  cargo run -- hello --help");

        Ok(())
    }
}
