//! Adapter implementations for TypeScript code generation.
//!
//! This module provides concrete implementations of the adapter traits
//! for TypeScript-specific frameworks: boune and bun:sqlite.

mod boune;
mod bun_sqlite;

pub use self::{boune::BouneAdapter, bun_sqlite::BunSqliteAdapter};
