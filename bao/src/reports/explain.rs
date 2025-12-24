//! Explain command report data structures.

use std::path::PathBuf;

use super::output::{Output, Report};

/// Report data from pipeline explanation.
#[derive(Debug)]
pub struct ExplainReport {
    /// Path to the manifest file.
    pub config_path: PathBuf,
    /// Manifest information.
    pub manifest: ManifestInfo,
    /// Pipeline phases.
    pub phases: Vec<PhaseInfo>,
    /// Validation lints.
    pub lints: Vec<LintInfo>,
    /// Analysis results from running the pipeline.
    pub analysis: AnalysisResult,
}

/// Information extracted from the manifest.
#[derive(Debug)]
pub struct ManifestInfo {
    /// CLI name.
    pub name: String,
    /// CLI version.
    pub version: String,
    /// Target language.
    pub language: String,
}

/// Information about a pipeline phase.
#[derive(Debug)]
pub struct PhaseInfo {
    /// Phase name.
    pub name: String,
    /// Phase description.
    pub description: String,
}

/// Information about a validation lint.
#[derive(Debug)]
pub struct LintInfo {
    /// Lint name.
    pub name: String,
    /// Lint description.
    pub description: String,
}

/// Analysis results from running the pipeline.
#[derive(Debug)]
pub struct AnalysisResult {
    /// Number of top-level commands.
    pub command_count: usize,
    /// Total number of handlers.
    pub handler_count: usize,
    /// Whether async is required.
    pub is_async: bool,
    /// Whether database is used.
    pub has_database: bool,
    /// Whether HTTP client is used.
    pub has_http: bool,
    /// Handler paths.
    pub handler_paths: Vec<String>,
    /// Context fields.
    pub context_fields: Vec<ContextFieldInfo>,
}

/// Information about a context field.
#[derive(Debug)]
pub struct ContextFieldInfo {
    /// Field name.
    pub name: String,
    /// Field type description.
    pub type_name: String,
    /// Environment variable.
    pub env_var: String,
}

impl Report for ExplainReport {
    fn render(&self, out: &mut dyn Output) {
        out.title("Bao Pipeline Explanation");
        out.newline();

        // Show manifest info
        out.key_value("Input", &self.config_path.display().to_string());
        out.key_value_indented("CLI name", &self.manifest.name);
        out.key_value_indented("Version", &self.manifest.version);
        out.key_value_indented("Language", &self.manifest.language);
        out.newline();

        // Show phases
        out.section("Pipeline Phases");
        for (i, phase) in self.phases.iter().enumerate() {
            out.numbered_item(i + 1, &format!("{} - {}", phase.name, phase.description));
        }
        out.newline();

        // Show lints
        out.section("Validation Lints");
        for lint in &self.lints {
            out.list_item(&format!("{}: {}", lint.name, lint.description));
        }
        out.newline();

        // Show analysis results
        out.section("Analysis Results");
        out.key_value_indented(
            "Commands",
            &format!(
                "{} top-level, {} total handlers",
                self.analysis.command_count, self.analysis.handler_count
            ),
        );
        out.key_value_indented("Async", if self.analysis.is_async { "yes" } else { "no" });
        out.key_value_indented(
            "Database",
            if self.analysis.has_database {
                "yes"
            } else {
                "no"
            },
        );
        out.key_value_indented(
            "HTTP client",
            if self.analysis.has_http { "yes" } else { "no" },
        );
        out.newline();

        // Show handler paths
        if !self.analysis.handler_paths.is_empty() {
            out.section("Handler Paths");
            for path in &self.analysis.handler_paths {
                out.list_item(&format!("src/handlers/{}.rs", path));
            }
            out.newline();
        }

        // Show context fields
        if !self.analysis.context_fields.is_empty() {
            out.section("Context Fields");
            for field in &self.analysis.context_fields {
                out.list_item(&format!("{} ({})", field.name, field.type_name));
                if !field.env_var.is_empty() {
                    out.key_value_indented("env", &field.env_var);
                }
            }
            out.newline();
        }

        // Show files to generate
        out.section("Files to Generate");
        out.list_item("[Config]         Cargo.toml / package.json");
        out.list_item("[Infrastructure] src/main.rs, src/app.rs, src/context.rs");
        out.list_item("[Generated]      src/generated/cli.rs, src/generated/commands/*.rs");
        out.list_item("[Handlers]       src/handlers/*.rs (only if missing)");
    }
}
