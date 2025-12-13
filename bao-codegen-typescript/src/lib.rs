//! TypeScript code generator for Bao CLI generator.
//!
//! This crate generates TypeScript CLI applications using [boune](https://www.npmjs.com/package/boune)
//! a CLI library targeting [Bun](https://bun.com/) runtime.
//!
//! # Usage
//!
//! This crate is used internally by the `baobao` CLI tool. You typically don't need
//! to use it directly.
//!
//! ```ignore
//! use baobao_codegen_typescript::Generator;
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
//! The generator produces a TypeScript CLI project structure:
//!
//! - `src/cli.ts` - Main CLI setup with boune
//! - `src/context.ts` - Shared context (database pools, HTTP clients)
//! - `src/index.ts` - Entry point
//! - `src/commands/*.ts` - Command definitions
//! - `src/handlers/*.ts` - Handler stubs for implementation
//! - `package.json`, `tsconfig.json`, `bao.toml`, `.gitignore`

mod code_file;
mod generator;
mod naming;
mod type_mapper;

pub mod adapters;
pub mod ast;
pub mod files;

pub use adapters::{BouneAdapter, BunSqliteAdapter};
pub use ast::{ArrowFn, Import, JsObject, MethodChain};
pub use baobao_codegen::language::{GenerateResult, LanguageCodegen, PreviewFile};
pub use code_file::{CodeFile, RawCode};
pub use generator::Generator;
pub use naming::TS_NAMING;
pub use type_mapper::TypeScriptTypeMapper;
