//! Clean operation - remove orphaned generated files.

use std::path::Path;

use baobao_codegen::pipeline::{Pipeline, Severity};
use baobao_manifest::Manifest;
use eyre::{Context, Result};

use crate::{language::LanguageSupport, reports::CleanReport};

/// Options for the clean operation.
pub struct CleanOptions<'a> {
    /// Output directory containing generated files.
    pub output_dir: &'a Path,
    /// Whether to preview without deleting.
    pub dry_run: bool,
}

/// Execute the clean operation.
///
/// Removes orphaned generated files that are no longer in the manifest.
pub fn clean(
    manifest: &Manifest,
    lang: LanguageSupport,
    opts: CleanOptions,
) -> Result<CleanReport> {
    // Run the pipeline
    let pipeline = Pipeline::new();
    let ctx = pipeline.run(manifest.clone()).wrap_err("Pipeline failed")?;

    // Collect warnings
    let warnings: Vec<String> = ctx
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, Severity::Warning))
        .map(|d| d.message.clone())
        .collect();

    // Get the generator and clean
    let generator = lang.generator(ctx);
    let result = if opts.dry_run {
        generator
            .preview_clean(opts.output_dir)
            .wrap_err("Failed to preview clean")?
    } else {
        generator
            .clean(opts.output_dir)
            .wrap_err("Failed to clean orphaned files")?
    };

    Ok(CleanReport {
        dry_run: opts.dry_run,
        warnings,
        deleted_commands: result.deleted_commands,
        deleted_handlers: result.deleted_handlers,
        skipped_handlers: result.skipped_handlers,
    })
}
