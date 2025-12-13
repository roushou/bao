//! Error handling adapter abstraction.
//!
//! This module defines the [`ErrorAdapter`] trait for abstracting error handling
//! patterns (eyre, anyhow, thiserror, etc.).

use super::cli::{Dependency, ImportSpec};

/// Trait for error handling adapters.
///
/// Implement this trait to support a specific error handling library.
pub trait ErrorAdapter {
    /// Adapter name for identification.
    fn name(&self) -> &'static str;

    /// Dependencies required by this error adapter.
    fn dependencies(&self) -> Vec<Dependency>;

    /// The Result type alias or full type (e.g., "eyre::Result<()>").
    fn result_type(&self, inner: &str) -> String;

    /// The unit Result type (e.g., "eyre::Result<()>").
    fn unit_result(&self) -> String {
        self.result_type("()")
    }

    /// Imports needed for error handling.
    fn imports(&self) -> Vec<ImportSpec>;

    /// The error conversion expression (e.g., `.wrap_err("message")`).
    fn wrap_error(&self, message: &str) -> Option<String>;
}
