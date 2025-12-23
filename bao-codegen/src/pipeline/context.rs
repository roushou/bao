//! Compilation context passed through pipeline phases.

use baobao_ir::AppIR;
use baobao_manifest::Manifest;

use super::diagnostic::{Diagnostic, Severity};
use crate::schema::ComputedData;

/// Context passed through all pipeline phases.
///
/// This struct carries the state of compilation through each phase,
/// accumulating results and diagnostics along the way.
#[derive(Debug)]
pub struct CompilationContext {
    /// The original manifest being compiled.
    pub manifest: Manifest,
    /// The lowered Application IR (populated by LowerPhase).
    pub ir: Option<AppIR>,
    /// Pre-computed analysis data (populated by AnalyzePhase).
    pub computed: Option<ComputedData>,
    /// Diagnostics collected during compilation.
    pub diagnostics: Vec<Diagnostic>,
}

impl CompilationContext {
    /// Create a new compilation context from a manifest.
    pub fn new(manifest: Manifest) -> Self {
        Self {
            manifest,
            ir: None,
            computed: None,
            diagnostics: Vec::new(),
        }
    }

    /// Check if any error diagnostics have been recorded.
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity.is_error())
    }

    /// Check if any warning diagnostics have been recorded.
    pub fn has_warnings(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity.is_warning())
    }

    /// Count the number of error diagnostics.
    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity.is_error())
            .count()
    }

    /// Count the number of warning diagnostics.
    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity.is_warning())
            .count()
    }

    /// Add an error diagnostic.
    pub fn add_error(&mut self, phase: &str, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::error(phase, message));
    }

    /// Add a warning diagnostic.
    pub fn add_warning(&mut self, phase: &str, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::warning(phase, message));
    }

    /// Add an info diagnostic.
    pub fn add_info(&mut self, phase: &str, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::info(phase, message));
    }

    /// Add a diagnostic with a location.
    pub fn add_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Get all error diagnostics.
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| matches!(d.severity, Severity::Error))
    }

    /// Get all warning diagnostics.
    pub fn warnings(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| matches!(d.severity, Severity::Warning))
    }

    /// Take the IR out of the context, consuming it.
    ///
    /// # Panics
    ///
    /// Panics if the IR has not been set (i.e., LowerPhase hasn't run).
    pub fn take_ir(&mut self) -> AppIR {
        self.ir.take().expect("IR not set - did LowerPhase run?")
    }

    /// Take the computed data out of the context, consuming it.
    ///
    /// # Panics
    ///
    /// Panics if computed data has not been set (i.e., AnalyzePhase hasn't run).
    pub fn take_computed(&mut self) -> ComputedData {
        self.computed
            .take()
            .expect("ComputedData not set - did AnalyzePhase run?")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_manifest(content: &str) -> Manifest {
        toml::from_str(content).expect("Failed to parse test manifest")
    }

    fn make_test_manifest() -> Manifest {
        parse_manifest(
            r#"
            [cli]
            name = "test"
            language = "rust"
        "#,
        )
    }

    #[test]
    fn test_context_creation() {
        let manifest = make_test_manifest();
        let ctx = CompilationContext::new(manifest);

        assert!(ctx.ir.is_none());
        assert!(ctx.computed.is_none());
        assert!(ctx.diagnostics.is_empty());
    }

    #[test]
    fn test_context_diagnostics() {
        let manifest = make_test_manifest();
        let mut ctx = CompilationContext::new(manifest);

        ctx.add_error("test", "test error");
        ctx.add_warning("test", "test warning");

        assert!(ctx.has_errors());
        assert!(ctx.has_warnings());
        assert_eq!(ctx.error_count(), 1);
        assert_eq!(ctx.warning_count(), 1);
    }

    #[test]
    fn test_context_no_errors() {
        let manifest = make_test_manifest();
        let mut ctx = CompilationContext::new(manifest);

        ctx.add_warning("test", "just a warning");
        ctx.add_info("test", "just info");

        assert!(!ctx.has_errors());
        assert!(ctx.has_warnings());
    }
}
