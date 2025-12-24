//! Core operations.
//!
//! This module contains the business logic for bao commands,
//! separated from CLI argument parsing and output rendering.

pub mod bake;
pub mod check;
pub mod clean;
pub mod explain;
pub mod info;

pub use bake::bake;
pub use check::check;
pub use clean::clean;
pub use explain::explain;
pub use info::info;
