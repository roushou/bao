//! Check operation - manifest validation.

use std::path::Path;

use baobao_codegen::pipeline::{Pipeline, Severity};
use baobao_manifest::Manifest;
use eyre::{Context, Result};

use crate::reports::CheckReport;

/// Execute the check operation.
///
/// Runs the pipeline to validate the manifest and returns diagnostics.
pub fn check(manifest: &Manifest, config_path: &Path) -> Result<CheckReport> {
    let pipeline = Pipeline::new();
    let ctx = pipeline
        .run(manifest.clone())
        .wrap_err("Validation failed")?;

    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut infos = Vec::new();

    for diag in &ctx.diagnostics {
        let msg = if let Some(loc) = &diag.location {
            format!("{}\n  --> {}", diag.message, loc)
        } else {
            diag.message.clone()
        };

        match diag.severity {
            Severity::Error => errors.push(msg),
            Severity::Warning => warnings.push(msg),
            Severity::Info => infos.push(msg),
        }
    }

    Ok(CheckReport {
        config_path: config_path.to_path_buf(),
        errors,
        warnings,
        infos,
    })
}
