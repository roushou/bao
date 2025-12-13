//! Eyre error handling adapter.

use baobao_codegen::adapters::{Dependency, ErrorAdapter, ImportSpec};

/// Eyre adapter for error handling.
#[derive(Debug, Clone, Default)]
pub struct EyreAdapter;

impl EyreAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl ErrorAdapter for EyreAdapter {
    fn name(&self) -> &'static str {
        "eyre"
    }

    fn dependencies(&self) -> Vec<Dependency> {
        vec![Dependency::new("eyre", "0.6")]
    }

    fn result_type(&self, inner: &str) -> String {
        format!("eyre::Result<{}>", inner)
    }

    fn imports(&self) -> Vec<ImportSpec> {
        vec![ImportSpec::new("eyre").symbol("Result")]
    }

    fn wrap_error(&self, message: &str) -> Option<String> {
        Some(format!(".wrap_err(\"{}\")", message))
    }
}
