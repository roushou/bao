//! Shared code generation utilities for Bao CLI generator.
//!
//! This crate provides language-agnostic abstractions and utilities
//! used by language-specific code generators (e.g., `baobao-codegen-rust`).
//!
//! # Module Organization
//!
//! - [`builder`] - Code generation building blocks (CodeBuilder, CodeFragment, etc.)
//! - [`schema`] - Schema traversal and info types (CommandTree, CommandInfo, etc.)
//! - [`generation`] - Output management (HandlerPaths, ImportCollector, etc.)
//! - [`language`] - Language-specific abstractions (LanguageCodegen, TypeMapper, etc.)
//! - [`testing`] - Test utilities (feature-gated)

pub mod builder;
pub mod generation;
pub mod language;
pub mod schema;

#[cfg(any(test, feature = "testing"))]
pub mod testing;
