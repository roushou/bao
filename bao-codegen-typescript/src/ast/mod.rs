//! TypeScript AST builders for generating types, functions, imports, and exports.
//!
//! These provide a high-level API for constructing TypeScript syntax,
//! which can then be rendered via CodeBuilder.

#![allow(dead_code)]
#![allow(unused)]

mod arrays;
mod chain;
mod consts;
mod exports;
mod fns;
mod imports;
mod interface;
mod objects;
mod types;

pub use arrays::JsArray;
pub use chain::MethodChain;
pub use consts::Const;
pub use exports::Export;
pub use fns::{Fn, Param};
pub use imports::Import;
pub use interface::Interface;
pub use objects::{ArrowFn, JsObject};
pub use types::{Field, ObjectType, TypeAlias, Union};
