use std::path::{Path, PathBuf};

use baobao_codegen::{generation::BaoToml, language::LanguageCodegen};
use baobao_codegen_rust::{
    Generator as RustGenerator,
    files::{CargoToml, GitIgnore as RustGitIgnore, MainRs},
};
use baobao_codegen_typescript::{
    Generator as TypeScriptGenerator,
    files::{GitIgnore as TsGitIgnore, IndexTs, PackageJson, TsConfig},
};
use baobao_core::{File, GeneratedFile};
use baobao_manifest::{Language, Manifest};
use clap::Args;
use dialoguer::{Select, theme::ColorfulTheme};
use eyre::{Context, Result};
use miette::Report;

#[derive(Args)]
pub struct InitCommand {
    /// Project name (defaults to current directory)
    #[arg(default_value = ".")]
    pub name: String,

    /// Output directory (defaults to ./<name>)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Target language for code generation
    #[arg(short, long)]
    pub language: Option<Language>,
}

impl InitCommand {
    pub fn run(&self) -> Result<()> {
        let (project_name, output_dir) = Self::resolve_paths(&self.name, self.output.clone())?;
        let language = match self.language {
            Some(lang) => lang,
            None => Self::prompt_language()?,
        };

        match language {
            Language::Rust => Self::create_rust_project(&project_name, &output_dir),
            Language::TypeScript => Self::create_typescript_project(&project_name, &output_dir),
        }
    }

    fn prompt_language() -> Result<Language> {
        let languages = ["Rust", "TypeScript"];
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select a language")
            .items(&languages)
            .default(0)
            .interact()
            .wrap_err("Failed to get language selection")?;

        Ok(match selection {
            0 => Language::Rust,
            _ => Language::TypeScript,
        })
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

    fn create_rust_project(name: &str, output_dir: &Path) -> Result<()> {
        // Create bao.toml
        BaoToml::new(name, Language::Rust).write(output_dir)?;

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
        RustGitIgnore.write(output_dir)?;

        // Create main.rs (not async for basic init)
        MainRs::new(false).write(output_dir)?;

        // Create handlers/hello.rs with a working example
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
        let schema = match Manifest::from_file(&bao_toml_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{:?}", Report::new(*e));
                std::process::exit(1);
            }
        };

        let generator = RustGenerator::new(&schema);
        let _ = generator
            .generate(output_dir)
            .wrap_err("Failed to generate code")?;

        println!("Created new Rust CLI project in {}", output_dir.display());
        println!();
        println!("Next steps:");
        if output_dir != Path::new(".") {
            println!("  cd {}", output_dir.display());
        }
        println!("  cargo run -- hello --help");

        Ok(())
    }

    fn create_typescript_project(name: &str, output_dir: &Path) -> Result<()> {
        // Create bao.toml
        BaoToml::new(name, Language::TypeScript).write(output_dir)?;

        // Create package.json
        PackageJson::new(name).write(output_dir)?;

        // Create tsconfig.json
        TsConfig.write(output_dir)?;

        // Create .gitignore
        TsGitIgnore.write(output_dir)?;

        // Create index.ts
        IndexTs.write(output_dir)?;

        // Create handlers/hello.ts with a working example
        std::fs::create_dir_all(output_dir.join("src").join("handlers"))?;
        File::new(
            output_dir.join("src").join("handlers").join("hello.ts"),
            r#"import type { Context } from "../context.ts";
import type { HelloArgs } from "../commands/hello.ts";

export async function run(ctx: Context, args: HelloArgs): Promise<void> {
  const name = args.name ?? "World";
  const greeting = `Hello, ${name}!`;

  if (args.uppercase) {
    console.log(greeting.toUpperCase());
  } else {
    console.log(greeting);
  }
}
"#,
        )
        .write()?;

        // Generate code from bao.toml
        let bao_toml_path = output_dir.join("bao.toml");
        let schema = match Manifest::from_file(&bao_toml_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{:?}", Report::new(*e));
                std::process::exit(1);
            }
        };

        let generator = TypeScriptGenerator::new(&schema);
        let _ = generator
            .generate(output_dir)
            .wrap_err("Failed to generate code")?;

        println!(
            "Created new TypeScript CLI project in {}",
            output_dir.display()
        );
        println!();
        println!("Next steps:");
        if output_dir != Path::new(".") {
            println!("  cd {}", output_dir.display());
        }
        println!("  bun install");
        println!("  bun run dev -- hello --help");

        Ok(())
    }
}
