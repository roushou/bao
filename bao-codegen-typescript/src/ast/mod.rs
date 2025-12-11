//! TypeScript AST builders for generating types, functions, imports, and exports.
//!
//! These provide a high-level API for constructing TypeScript syntax,
//! which can then be rendered via CodeBuilder.

#![allow(dead_code)]
#![allow(unused)]

mod chain;
mod consts;
mod exports;
mod fns;
mod imports;
mod interface;
mod objects;
mod types;

pub use chain::MethodChain;
pub(crate) use consts::Const;
pub(crate) use exports::Export;
pub(crate) use fns::{Fn, Param};
pub(crate) use imports::Import;
pub(crate) use interface::Interface;
pub(crate) use objects::{ArrowFn, JsObject};
pub(crate) use types::{Field, ObjectType, TypeAlias, Union};
