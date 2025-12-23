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
//!
//! # Declarative Type Specifications
//!
//! - [`StructSpec`], [`EnumSpec`] - Type definitions
//! - [`FunctionSpec`] - Function and method definitions
//! - [`TypeRef`] - Language-agnostic type references
//! - [`TypeMapper`] - Trait for language-specific type rendering

mod code_builder;
mod expr;
mod file_builder;
mod function;
mod indent;
mod renderable;
mod structure;
mod types;

pub use code_builder::CodeBuilder;
pub use expr::{
    Binding, Block, BuilderSpec, Constructor, MethodCall, RenderOptions, Renderer, Terminal, Value,
};
pub use file_builder::FileBuilder;
pub use function::{
    FunctionRenderer, FunctionSpec, GenericParam, MatchArm, ParamSpec, Pattern, Receiver, Statement,
};
pub use indent::Indent;
pub use renderable::{CodeFragment, Renderable};
pub use structure::{
    AttributeArg, AttributeSpec, EnumSpec, FieldSpec, StructSpec, StructureRenderer, VariantKind,
    VariantSpec,
};
pub use types::{PrimitiveType, TypeMapper, TypeRef, Visibility};
