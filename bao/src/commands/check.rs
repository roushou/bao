use std::path::PathBuf;

use baobao_codegen::pipeline::{Pipeline, Severity};
use baobao_manifest::BaoToml;
use clap::Args;
use eyre::{Context, Result};

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

        // Run the pipeline to validate
        let pipeline = Pipeline::new();
        let ctx = pipeline.run(schema.clone()).wrap_err("Validation failed")?;

        // Print all diagnostics
        let mut has_errors = false;
        let mut has_warnings = false;
        for diag in &ctx.diagnostics {
            match diag.severity {
                Severity::Error => {
                    has_errors = true;
                    eprintln!("error: {}", diag.message);
                    if let Some(loc) = &diag.location {
                        eprintln!("  --> {}", loc);
                    }
                }
                Severity::Warning => {
                    has_warnings = true;
                    eprintln!("warning: {}", diag.message);
                    if let Some(loc) = &diag.location {
                        eprintln!("  --> {}", loc);
                    }
                }
                Severity::Info => {
                    println!("info: {}", diag.message);
                    if let Some(loc) = &diag.location {
                        println!("  --> {}", loc);
                    }
                }
            }
        }

        if has_errors {
            std::process::exit(1);
        }

        if has_warnings {
            println!();
        }

        println!("✓ {} is valid\n", self.config.display());

        Ok(())
    }
}
