use std::path::PathBuf;

use baobao_codegen::pipeline::{Pipeline, phases::ValidatePhase};
use baobao_core::{ContextFieldType, DatabaseType};
use baobao_manifest::{BaoToml, Language};
use clap::Args;
use eyre::{Context, Result};

use super::UnwrapOrExit;

#[derive(Args)]
pub struct ExplainCommand {
    /// Path to bao.toml (defaults to ./bao.toml)
    #[arg(short, long, default_value = "bao.toml")]
    pub config: PathBuf,
}

impl ExplainCommand {
    pub fn run(&self) -> Result<()> {
        let bao_toml = BaoToml::open(&self.config).unwrap_or_exit();
        let schema = bao_toml.schema();

        // Get pipeline and validation phase info
        let pipeline = Pipeline::new();
        let validate_phase = ValidatePhase::new();

        println!("Bao Pipeline Explanation");
        println!("========================");
        println!();

        // Show manifest info
        println!("Input: {}", self.config.display());
        println!("  CLI name: {}", schema.cli.name);
        println!("  Version: {}", schema.cli.version);
        println!("  Language: {}", language_name(schema.cli.language));
        println!();

        // Show phases (descriptions come from the Phase trait)
        println!("Pipeline Phases:");
        for (i, phase) in pipeline.phase_info().iter().enumerate() {
            println!("  {}. {} - {}", i + 1, phase.name, phase.description);
        }
        println!();

        // Show lints (descriptions come from the Lint trait)
        println!("Validation Lints:");
        for lint in validate_phase.lint_info() {
            println!("  - {}: {}", lint.name, lint.description);
        }
        println!();

        // Run pipeline to get computed data
        let ctx = pipeline.run(schema.clone()).wrap_err("Pipeline failed")?;
        let computed = ctx.computed.as_ref().expect("ComputedData should be set");

        // Show what will be generated
        println!("Analysis Results:");
        println!(
            "  Commands: {} top-level, {} total handlers",
            computed.command_count, computed.handler_count
        );
        println!("  Async: {}", if computed.is_async { "yes" } else { "no" });
        println!(
            "  Database: {}",
            if computed.has_database { "yes" } else { "no" }
        );
        println!(
            "  HTTP client: {}",
            if computed.has_http { "yes" } else { "no" }
        );
        println!();

        // Show handler paths
        if !computed.command_paths.is_empty() {
            println!("Handler Paths:");
            let mut paths: Vec<_> = computed.command_paths.iter().collect();
            paths.sort();
            for path in paths {
                println!("  src/handlers/{}.rs", path);
            }
            println!();
        }

        // Show context fields
        if !computed.context_fields.is_empty() {
            println!("Context Fields:");
            for field in &computed.context_fields {
                println!(
                    "  - {} ({})",
                    field.name,
                    field_type_name(&field.field_type)
                );
                if !field.env_var.is_empty() {
                    println!("    env: {}", field.env_var);
                }
            }
            println!();
        }

        // Show files that will be generated
        println!("Files to Generate:");
        println!("  [Config]         Cargo.toml / package.json");
        println!("  [Infrastructure] src/main.rs, src/app.rs, src/context.rs");
        println!("  [Generated]      src/generated/cli.rs, src/generated/commands/*.rs");
        println!("  [Handlers]       src/handlers/*.rs (only if missing)");

        Ok(())
    }
}

fn language_name(lang: Language) -> &'static str {
    match lang {
        Language::Rust => "Rust",
        Language::TypeScript => "TypeScript",
    }
}

fn field_type_name(field_type: &ContextFieldType) -> &'static str {
    match field_type {
        ContextFieldType::Database(db_type) => match db_type {
            DatabaseType::Postgres => "PostgreSQL",
            DatabaseType::Mysql => "MySQL",
            DatabaseType::Sqlite => "SQLite",
        },
        ContextFieldType::Http => "HTTP client",
    }
}
