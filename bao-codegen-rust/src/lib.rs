//! Rust code generator for Bao CLI generator.
//!
//! This crate generates Rust CLI applications using [clap](https://crates.io/crates/clap)
//! for argument parsing.
//!
//! # Usage
//!
//! This crate is used internally by the `baobao` CLI tool. You typically don't need
//! to use it directly.
//!
//! ```ignore
//! use baobao_codegen_rust::Generator;
//! use baobao_codegen::LanguageCodegen;
//! use baobao_manifest::Manifest;
//! use std::path::Path;
//!
//! let manifest = Manifest::from_file("bao.toml")?;
//! let generator = Generator::new(&manifest);
//!
//! // Preview files without writing
//! let files = generator.preview();
//!
//! // Generate files to disk
//! let result = generator.generate(Path::new("output"))?;
//! ```
//!
//! # Generated Output
//!
//! The generator produces a Rust CLI project structure:
//!
//! - `src/cli.rs` - CLI definition with clap derive macros
//! - `src/context.rs` - Shared context (database pools, HTTP clients)
//! - `src/main.rs` - Entry point and command dispatch
//! - `src/commands/*.rs` - Command modules
//! - `src/handlers/*.rs` - Handler stubs for implementation
//! - `Cargo.toml`, `bao.toml`, `.gitignore`

mod generator;
mod naming;
mod render;
mod rust_file;
mod type_mapper;

pub mod ast;
pub mod files;

pub use ast::{Arm, Enum, Field, Fn, Impl, Match, Param, Struct, Variant};
pub use baobao_codegen::{GenerateResult, LanguageCodegen, PreviewFile};
pub use generator::Generator;
pub use naming::RUST_NAMING;
pub use render::{RustFileBuilder, render_imports};
pub use rust_file::{RawCode, RustFile, Use};
pub use type_mapper::RustTypeMapper;
