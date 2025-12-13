//! Tokio async runtime adapter.

use baobao_codegen::{
    adapters::{Dependency, ImportSpec, RuntimeAdapter, RuntimeInfo},
    builder::CodeFragment,
};

/// Tokio adapter for async runtime.
#[derive(Debug, Clone, Default)]
pub struct TokioAdapter;

impl TokioAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl RuntimeAdapter for TokioAdapter {
    fn name(&self) -> &'static str {
        "tokio"
    }

    fn dependencies(&self) -> Vec<Dependency> {
        vec![Dependency::new(
            "tokio",
            r#"{ version = "1", features = ["rt-multi-thread", "macros"] }"#,
        )]
    }

    fn main_attribute(&self) -> Option<String> {
        Some("tokio::main".to_string())
    }

    fn generate_init(&self, _info: &RuntimeInfo) -> Option<Vec<CodeFragment>> {
        // Tokio uses attribute macro, no explicit init code needed
        None
    }

    fn imports(&self) -> Vec<ImportSpec> {
        // Tokio main attribute doesn't require explicit imports
        vec![]
    }
}
