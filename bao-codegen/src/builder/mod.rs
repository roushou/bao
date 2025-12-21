//! Code generation building blocks.
//!
//! This module provides the core primitives for generating code:
//! - [`CodeBuilder`] - Fluent API for building indented code
//! - [`CodeFragment`] - Intermediate representation for code pieces
//! - [`Renderable`] - Trait for types that can be converted to code fragments
//! - [`FileBuilder`] - Composition of imports and code
//! - [`Indent`] - Indentation configuration
//!
//! # Language-Agnostic Expression Builders
//!
//! - [`Value`] - Semantic values (bool, int, duration, enum variants, etc.)
//! - [`BuilderSpec`] - Declarative specification for builder/fluent API patterns
//! - [`Block`] - Scoped expressions with let bindings
//! - [`Renderer`] - Trait for language-specific rendering

mod code_builder;
mod expr;
mod file_builder;
mod indent;
mod renderable;

pub use code_builder::CodeBuilder;
pub use expr::{
    Binding, Block, BuilderSpec, Constructor, MethodCall, RenderExt, RenderOptions, Renderer,
    Terminal, Value,
};
pub use file_builder::FileBuilder;
pub use indent::Indent;
pub use renderable::{CodeFragment, Renderable};
