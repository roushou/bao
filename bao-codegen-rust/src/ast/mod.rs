//! Rust AST builders for generating structs, enums, impls, and functions.
//!
//! These provide a high-level API for constructing Rust syntax,
//! which can then be rendered via CodeBuilder.

mod chains;
mod enums;
mod fns;
mod impls;
mod structs;

pub use chains::MethodChain;
pub use enums::{Enum, Variant};
pub use fns::{Arm, Fn, Match, Param};
pub use impls::Impl;
pub use structs::{Field, Struct};
