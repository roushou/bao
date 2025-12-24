//! Explain operation - pipeline explanation.

use std::path::Path;

use baobao_codegen::pipeline::{Pipeline, phases::ValidatePhase};
use baobao_core::{ContextFieldType, DatabaseType};
use baobao_manifest::{Language, Manifest};
use eyre::{Context, Result};

use crate::reports::{
    AnalysisResult, ContextFieldInfo, ExplainReport, LintInfo, ManifestInfo, PhaseInfo,
};

/// Execute the explain operation.
///
/// Runs the pipeline and returns information about what it does.
pub fn explain(manifest: &Manifest, config_path: &Path) -> Result<ExplainReport> {
    // Get pipeline and validation phase info
    let pipeline = Pipeline::new();
    let validate_phase = ValidatePhase::new();

    // Collect phase info
    let phases: Vec<PhaseInfo> = pipeline
        .phase_info()
        .into_iter()
        .map(|p| PhaseInfo {
            name: p.name.to_string(),
            description: p.description.to_string(),
        })
        .collect();

    // Collect lint info
    let lints: Vec<LintInfo> = validate_phase
        .lint_info()
        .into_iter()
        .map(|l| LintInfo {
            name: l.name.to_string(),
            description: l.description.to_string(),
        })
        .collect();

    // Run pipeline to get computed data
    let ctx = pipeline.run(manifest.clone()).wrap_err("Pipeline failed")?;
    let computed = ctx.computed.as_ref().expect("ComputedData should be set");

    // Collect handler paths (sorted)
    let mut handler_paths: Vec<String> = computed.command_paths.iter().cloned().collect();
    handler_paths.sort();

    // Collect context fields
    let context_fields: Vec<ContextFieldInfo> = computed
        .context_fields
        .iter()
        .map(|f| ContextFieldInfo {
            name: f.name.clone(),
            type_name: field_type_name(&f.field_type).to_string(),
            env_var: f.env_var.clone(),
        })
        .collect();

    Ok(ExplainReport {
        config_path: config_path.to_path_buf(),
        manifest: ManifestInfo {
            name: manifest.cli.name.clone(),
            version: manifest.cli.version.to_string(),
            language: language_name(manifest.cli.language).to_string(),
        },
        phases,
        lints,
        analysis: AnalysisResult {
            command_count: computed.command_count,
            handler_count: computed.handler_count,
            is_async: computed.is_async,
            has_database: computed.has_database,
            has_http: computed.has_http,
            handler_paths,
            context_fields,
        },
    })
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
