//! Database adapter abstraction.
//!
//! This module defines the [`DatabaseAdapter`] trait for abstracting database
//! connection and pool code generation (sqlx, diesel, bun:sqlite, etc.).

use baobao_ir::DatabaseType;
// Re-export IR types for convenience
pub use baobao_ir::{PoolConfig, SqliteOptions};

use super::cli::{Dependency, ImportSpec};
use crate::builder::Value;

/// Info needed to generate pool initialization.
#[derive(Debug, Clone)]
pub struct PoolInitInfo {
    /// Field name in the context struct
    pub field_name: String,
    /// Database type
    pub db_type: DatabaseType,
    /// Environment variable for connection string
    pub env_var: String,
    /// Pool configuration
    pub pool_config: PoolConfig,
    /// SQLite-specific config (only for SQLite)
    pub sqlite_config: Option<SqliteOptions>,
}

/// Trait for database adapters.
///
/// Implement this trait to support a specific database library (sqlx, diesel, etc.).
pub trait DatabaseAdapter {
    /// Adapter name for identification.
    fn name(&self) -> &'static str;

    /// Dependencies required for a specific database type.
    fn dependencies(&self, db_type: DatabaseType) -> Vec<Dependency>;

    /// The type name for a connection/pool.
    fn pool_type(&self, db_type: DatabaseType) -> &'static str;

    /// Generate pool/connection initialization as a semantic Value.
    ///
    /// Returns a `Value` that represents the initialization expression.
    /// The caller is responsible for rendering it with the appropriate language renderer.
    fn pool_init(&self, info: &PoolInitInfo) -> Value;

    /// Imports needed for database code.
    fn imports(&self, db_type: DatabaseType) -> Vec<ImportSpec>;

    /// Whether this adapter requires async initialization.
    fn requires_async(&self, db_type: DatabaseType) -> bool;
}
