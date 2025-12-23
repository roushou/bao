use std::path::PathBuf;

use baobao_codegen::{
    language::LanguageCodegen,
    pipeline::{Pipeline, Severity},
};
use baobao_codegen_rust::Generator as RustGenerator;
use baobao_codegen_typescript::Generator as TypeScriptGenerator;
use baobao_manifest::{BaoToml, Language};
use clap::Args;
use eyre::{Context, Result};

use super::UnwrapOrExit;

#[derive(Args)]
pub struct CleanCommand {
    /// Path to bao.toml (defaults to ./bao.toml)
    #[arg(short, long, default_value = "bao.toml")]
    pub config: PathBuf,

    /// Output directory (defaults to current directory)
    #[arg(short, long, default_value = ".")]
    pub output: PathBuf,

    /// Preview what would be deleted without actually deleting
    #[arg(long)]
    pub dry_run: bool,
}

impl CleanCommand {
    pub fn run(&self) -> Result<()> {
        let bao_toml = BaoToml::open(&self.config).unwrap_or_exit();
        let schema = bao_toml.schema();
        let language = schema.cli.language;

        // Run the pipeline to validate, lower, and analyze
        let pipeline = Pipeline::new();
        let ctx = pipeline.run(schema.clone()).wrap_err("Pipeline failed")?;

        // Print any warnings
        for diag in &ctx.diagnostics {
            if matches!(diag.severity, Severity::Warning) {
                eprintln!("warning: {}", diag.message);
            }
        }

        let result = match language {
            Language::Rust => {
                let generator = RustGenerator::from_context(ctx);
                if self.dry_run {
                    generator
                        .preview_clean(&self.output)
                        .wrap_err("Failed to preview clean")?
                } else {
                    generator
                        .clean(&self.output)
                        .wrap_err("Failed to clean orphaned files")?
                }
            }
            Language::TypeScript => {
                let generator = TypeScriptGenerator::from_context(ctx);
                if self.dry_run {
                    generator
                        .preview_clean(&self.output)
                        .wrap_err("Failed to preview clean")?
                } else {
                    generator
                        .clean(&self.output)
                        .wrap_err("Failed to clean orphaned files")?
                }
            }
        };

        let has_deletions =
            !result.deleted_commands.is_empty() || !result.deleted_handlers.is_empty();
        let has_skipped = !result.skipped_handlers.is_empty();

        if !has_deletions && !has_skipped {
            println!("No orphaned files found.");
            return Ok(());
        }

        if self.dry_run {
            if has_deletions {
                println!("Would delete:");
                for path in &result.deleted_commands {
                    println!("  - {}", path);
                }
                for path in &result.deleted_handlers {
                    println!("  - {}", path);
                }
            }
        } else if has_deletions {
            println!("Deleted:");
            for path in &result.deleted_commands {
                println!("  - {}", path);
            }
            for path in &result.deleted_handlers {
                println!("  - {}", path);
            }
        }

        if has_skipped {
            println!();
            println!("Skipped (modified by user):");
            for path in &result.skipped_handlers {
                println!("  ! {}", path);
            }
        }

        Ok(())
    }
}
