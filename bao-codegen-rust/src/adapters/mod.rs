//! Adapter implementations for Rust code generation.
//!
//! This module provides concrete implementations of the adapter traits
//! for Rust-specific frameworks: clap, sqlx, tokio, and eyre.

mod clap;
mod eyre;
mod sqlx;
mod tokio;

pub use self::{clap::ClapAdapter, eyre::EyreAdapter, sqlx::SqlxAdapter, tokio::TokioAdapter};
