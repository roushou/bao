//! Compilation pipeline for code generation.
//!
//! This module provides a [`Pipeline`] orchestrator that manages the compilation
//! phases from manifest parsing to code generation. The pipeline provides:
//!
//! - Explicit phase boundaries (validate → lower → analyze)
//! - Plugin hooks for extensibility (before/after each phase)
//! - Unified diagnostics collection
//! - Shared computation via [`CompilationContext`]
//!
//! # Example
//!
//! ```ignore
//! use baobao_codegen::pipeline::Pipeline;
//!
//! let pipeline = Pipeline::new();
//! let ctx = pipeline.run(manifest)?;
//!
//! // Check for warnings
//! for diag in &ctx.diagnostics {
//!     if matches!(diag.severity, Severity::Warning) {
//!         eprintln!("warning: {}", diag.message);
//!     }
//! }
//!
//! // Use context with generator
//! let generator = RustGenerator::from_context(ctx);
//! ```

mod context;
mod diagnostic;
mod phase;
pub mod phases;
mod plugin;
mod runner;

pub use context::CompilationContext;
pub use diagnostic::{Diagnostic, Severity};
pub use phase::Phase;
pub use plugin::Plugin;
pub use runner::Pipeline;
