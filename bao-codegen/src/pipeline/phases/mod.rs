//! Built-in pipeline phases.
//!
//! This module provides the standard phases that run in every pipeline:
//!
//! - [`ValidatePhase`] - validates the manifest and collects diagnostics
//! - [`LowerPhase`] - transforms manifest to Application IR
//! - [`AnalyzePhase`] - computes shared data from IR

mod analyze;
mod lower;
mod validate;

pub use analyze::AnalyzePhase;
pub use lower::LowerPhase;
pub use validate::{
    CommandNamingLint, DuplicateCommandLint, EmptyDescriptionLint, Lint, ValidatePhase,
};
