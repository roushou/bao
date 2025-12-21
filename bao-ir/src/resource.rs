//! Resource configuration types.
//!
//! These types represent the unified configuration for database pools,
//! SQLite options, and other resources. They serve as the single source
//! of truth for code generation, eliminating duplication between crates.

use std::time::Duration;

/// Connection pool configuration.
///
/// This is the unified type for pool configuration, replacing the duplicate
/// `PoolConfigInfo` (bao-codegen) and `PoolConfig` (bao-codegen adapters).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PoolConfig {
    /// Maximum number of connections in the pool.
    pub max_connections: Option<u32>,
    /// Minimum number of connections to maintain.
    pub min_connections: Option<u32>,
    /// Timeout for acquiring a connection from the pool.
    pub acquire_timeout: Option<Duration>,
    /// Maximum time a connection can remain idle before being closed.
    pub idle_timeout: Option<Duration>,
    /// Maximum lifetime of a connection.
    pub max_lifetime: Option<Duration>,
}

impl PoolConfig {
    /// Returns true if any pool option is configured.
    pub fn has_config(&self) -> bool {
        self.max_connections.is_some()
            || self.min_connections.is_some()
            || self.acquire_timeout.is_some()
            || self.idle_timeout.is_some()
            || self.max_lifetime.is_some()
    }
}

/// SQLite-specific configuration options.
///
/// This is the unified type for SQLite configuration, replacing the duplicate
/// `SqliteConfigInfo` (bao-codegen) and `SqliteConfig` (bao-codegen adapters).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SqliteOptions {
    /// Direct file path to the SQLite database.
    pub path: Option<String>,
    /// Create the database file if it doesn't exist.
    pub create_if_missing: Option<bool>,
    /// Open the database in read-only mode.
    pub read_only: Option<bool>,
    /// Journal mode.
    pub journal_mode: Option<JournalMode>,
    /// Synchronous mode.
    pub synchronous: Option<SynchronousMode>,
    /// Busy timeout.
    pub busy_timeout: Option<Duration>,
    /// Enable foreign key constraints.
    pub foreign_keys: Option<bool>,
}

impl SqliteOptions {
    /// Returns true if any SQLite-specific option is configured.
    pub fn has_config(&self) -> bool {
        self.create_if_missing.is_some()
            || self.read_only.is_some()
            || self.journal_mode.is_some()
            || self.synchronous.is_some()
            || self.busy_timeout.is_some()
            || self.foreign_keys.is_some()
    }
}

/// SQLite journal mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum JournalMode {
    #[default]
    Wal,
    Delete,
    Truncate,
    Persist,
    Memory,
    Off,
}

impl JournalMode {
    /// Get the PascalCase string representation for code generation.
    pub fn as_str(&self) -> &'static str {
        match self {
            JournalMode::Wal => "Wal",
            JournalMode::Delete => "Delete",
            JournalMode::Truncate => "Truncate",
            JournalMode::Persist => "Persist",
            JournalMode::Memory => "Memory",
            JournalMode::Off => "Off",
        }
    }
}

/// SQLite synchronous mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SynchronousMode {
    Off,
    Normal,
    #[default]
    Full,
    Extra,
}

impl SynchronousMode {
    /// Get the PascalCase string representation for code generation.
    pub fn as_str(&self) -> &'static str {
        match self {
            SynchronousMode::Off => "Off",
            SynchronousMode::Normal => "Normal",
            SynchronousMode::Full => "Full",
            SynchronousMode::Extra => "Extra",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_config_has_config() {
        let empty = PoolConfig::default();
        assert!(!empty.has_config());

        let with_max = PoolConfig {
            max_connections: Some(10),
            ..Default::default()
        };
        assert!(with_max.has_config());

        let with_timeout = PoolConfig {
            acquire_timeout: Some(Duration::from_secs(30)),
            ..Default::default()
        };
        assert!(with_timeout.has_config());
    }

    #[test]
    fn test_sqlite_options_has_config() {
        let empty = SqliteOptions::default();
        assert!(!empty.has_config());

        let with_journal = SqliteOptions {
            journal_mode: Some(JournalMode::Wal),
            ..Default::default()
        };
        assert!(with_journal.has_config());

        // path alone doesn't count as "config" (it's required for connection)
        let with_path_only = SqliteOptions {
            path: Some("db.sqlite".to_string()),
            ..Default::default()
        };
        assert!(!with_path_only.has_config());
    }

    #[test]
    fn test_journal_mode_as_str() {
        assert_eq!(JournalMode::Wal.as_str(), "Wal");
        assert_eq!(JournalMode::Delete.as_str(), "Delete");
        assert_eq!(JournalMode::Truncate.as_str(), "Truncate");
        assert_eq!(JournalMode::Persist.as_str(), "Persist");
        assert_eq!(JournalMode::Memory.as_str(), "Memory");
        assert_eq!(JournalMode::Off.as_str(), "Off");
    }

    #[test]
    fn test_synchronous_mode_as_str() {
        assert_eq!(SynchronousMode::Off.as_str(), "Off");
        assert_eq!(SynchronousMode::Normal.as_str(), "Normal");
        assert_eq!(SynchronousMode::Full.as_str(), "Full");
        assert_eq!(SynchronousMode::Extra.as_str(), "Extra");
    }

    #[test]
    fn test_default_values() {
        assert_eq!(JournalMode::default(), JournalMode::Wal);
        assert_eq!(SynchronousMode::default(), SynchronousMode::Full);
    }
}
