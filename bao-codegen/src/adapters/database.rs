//! Database adapter abstraction.
//!
//! This module defines the [`DatabaseAdapter`] trait for abstracting database
//! connection and pool code generation (sqlx, diesel, bun:sqlite, etc.).

use baobao_core::DatabaseType;

use super::cli::{Dependency, ImportSpec};
use crate::builder::CodeFragment;

/// Pool configuration options.
#[derive(Debug, Clone, Default)]
pub struct PoolConfig {
    /// Maximum number of connections
    pub max_connections: Option<u32>,
    /// Minimum number of connections
    pub min_connections: Option<u32>,
    /// Connection acquire timeout in seconds
    pub acquire_timeout: Option<u64>,
    /// Idle connection timeout in seconds
    pub idle_timeout: Option<u64>,
    /// Maximum connection lifetime in seconds
    pub max_lifetime: Option<u64>,
}

impl PoolConfig {
    /// Check if any pool options are configured.
    pub fn has_config(&self) -> bool {
        self.max_connections.is_some()
            || self.min_connections.is_some()
            || self.acquire_timeout.is_some()
            || self.idle_timeout.is_some()
            || self.max_lifetime.is_some()
    }
}

/// SQLite-specific configuration options.
#[derive(Debug, Clone, Default)]
pub struct SqliteConfig {
    /// Direct file path (takes precedence over env var)
    pub path: Option<String>,
    /// Create database if it doesn't exist
    pub create_if_missing: Option<bool>,
    /// Open in read-only mode
    pub read_only: Option<bool>,
    /// Journal mode (wal, delete, truncate, persist, memory, off)
    pub journal_mode: Option<String>,
    /// Synchronous mode (off, normal, full, extra)
    pub synchronous: Option<String>,
    /// Busy timeout in milliseconds
    pub busy_timeout: Option<u64>,
    /// Enable foreign key constraints
    pub foreign_keys: Option<bool>,
}

impl SqliteConfig {
    /// Check if any SQLite options are configured.
    pub fn has_config(&self) -> bool {
        self.create_if_missing.is_some()
            || self.read_only.is_some()
            || self.journal_mode.is_some()
            || self.synchronous.is_some()
            || self.busy_timeout.is_some()
            || self.foreign_keys.is_some()
    }
}

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
    pub sqlite_config: Option<SqliteConfig>,
    /// Whether initialization is async
    pub is_async: bool,
}

/// Info for generating database-specific options.
#[derive(Debug, Clone)]
pub struct DatabaseOptionsInfo {
    /// Database type
    pub db_type: DatabaseType,
    /// SQLite configuration
    pub sqlite: Option<SqliteConfig>,
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

    /// Generate pool/connection initialization code.
    fn generate_pool_init(&self, info: &PoolInitInfo) -> Vec<CodeFragment>;

    /// Generate database-specific options (e.g., SQLite connect options).
    fn generate_options(&self, info: &DatabaseOptionsInfo) -> Option<Vec<CodeFragment>>;

    /// Imports needed for database code.
    fn imports(&self, db_type: DatabaseType) -> Vec<ImportSpec>;

    /// Whether this adapter requires async initialization.
    fn requires_async(&self, db_type: DatabaseType) -> bool;
}
