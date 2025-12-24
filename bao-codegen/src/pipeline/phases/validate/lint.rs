//! Lint trait for manifest validation.

use baobao_manifest::Manifest;

use crate::pipeline::Diagnostic;

/// Information about a lint.
#[derive(Debug, Clone)]
pub struct LintInfo {
    /// The lint name.
    pub name: &'static str,
    /// A human-readable description.
    pub description: &'static str,
}

/// A lint that checks the manifest for issues.
pub trait Lint: Send + Sync {
    /// The name of this lint.
    fn name(&self) -> &'static str;

    /// A human-readable description of what this lint checks.
    fn description(&self) -> &'static str;

    /// Check the manifest and add any diagnostics.
    fn check(&self, manifest: &Manifest, diagnostics: &mut Vec<Diagnostic>);

    /// Get information about this lint.
    fn info(&self) -> LintInfo {
        LintInfo {
            name: self.name(),
            description: self.description(),
        }
    }
}
