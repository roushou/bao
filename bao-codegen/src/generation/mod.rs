//! Code generation outputs and file management.
//!
//! This module provides utilities for managing generated output:
//! - [`HandlerPaths`] - Handler file path computation and orphan detection
//! - [`ImportCollector`] - Import tracking and deduplication
//! - [`DependencyCollector`] - Package dependency tracking
//! - [`BaoToml`] - bao.toml configuration file generation

mod bao_toml;
mod handlers;
mod imports;

pub use bao_toml::BaoToml;
pub use handlers::{HandlerPaths, OrphanHandler, find_orphan_commands};
pub use imports::{DependencyCollector, DependencySpec, ImportCollector};
