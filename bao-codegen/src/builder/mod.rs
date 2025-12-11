//! Code generation building blocks.
//!
//! This module provides the core primitives for generating code:
//! - [`CodeBuilder`] - Fluent API for building indented code
//! - [`CodeFragment`] - Intermediate representation for code pieces
//! - [`Renderable`] - Trait for types that can be converted to code fragments
//! - [`FileBuilder`] - Composition of imports and code
//! - [`Indent`] - Indentation configuration

mod code_builder;
mod file_builder;
mod indent;
mod renderable;

pub use code_builder::CodeBuilder;
pub use file_builder::FileBuilder;
pub use indent::Indent;
pub use renderable::{CodeFragment, Renderable};
