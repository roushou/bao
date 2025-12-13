//! Async runtime adapter abstraction.
//!
//! This module defines the [`RuntimeAdapter`] trait for abstracting async
//! runtime setup (tokio, async-std, smol, etc.).

use super::cli::{Dependency, ImportSpec};
use crate::builder::CodeFragment;

/// Info for generating async runtime setup.
#[derive(Debug, Clone)]
pub struct RuntimeInfo {
    /// Whether the application uses async
    pub is_async: bool,
    /// Whether multi-threaded runtime is needed
    pub multi_threaded: bool,
}

/// Trait for async runtime adapters.
///
/// Implement this trait to support a specific async runtime (tokio, async-std, etc.).
pub trait RuntimeAdapter {
    /// Adapter name for identification.
    fn name(&self) -> &'static str;

    /// Dependencies required by this runtime.
    fn dependencies(&self) -> Vec<Dependency>;

    /// Attribute to apply to async main function (e.g., `#[tokio::main]`).
    fn main_attribute(&self) -> Option<String>;

    /// Generate any runtime initialization code.
    fn generate_init(&self, info: &RuntimeInfo) -> Option<Vec<CodeFragment>>;

    /// Imports needed for runtime code.
    fn imports(&self) -> Vec<ImportSpec>;
}
