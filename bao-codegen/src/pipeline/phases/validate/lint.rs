//! Lint trait for manifest validation.

use baobao_manifest::Manifest;

use crate::pipeline::Diagnostic;

/// A lint that checks the manifest for issues.
pub trait Lint: Send + Sync {
    /// The name of this lint.
    fn name(&self) -> &'static str;

    /// Check the manifest and add any diagnostics.
    fn check(&self, manifest: &Manifest, diagnostics: &mut Vec<Diagnostic>);
}
