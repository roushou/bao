use serde::{Deserialize, Serialize};

use super::PoolConfig;

/// Configuration for SQLite database
#[derive(Debug, Deserialize, Clone, Default)]
pub struct SqliteConfig {
    /// Direct file path to the SQLite database (e.g., "db.sqlite")
    pub path: Option<String>,

    /// Environment variable for database path (ignored if `path` is set)
    pub env: Option<String>,

    /// Pool configuration
    #[serde(flatten)]
    pub pool: PoolConfig,

    /// Create the database file if it doesn't exist (default: true)
    pub create_if_missing: Option<bool>,

    /// Open the database in read-only mode (default: false)
    pub read_only: Option<bool>,

    /// Journal mode: wal, delete, truncate, persist, memory, off (default: wal)
    pub journal_mode: Option<JournalMode>,

    /// Synchronous mode: full, normal, off (default: full)
    pub synchronous: Option<SynchronousMode>,

    /// Busy timeout in milliseconds (default: 5000)
    pub busy_timeout: Option<u64>,

    /// Enable foreign key constraints (default: true)
    pub foreign_keys: Option<bool>,
}

impl SqliteConfig {
    /// Returns true if any SQLite-specific option is configured
    pub fn has_sqlite_options(&self) -> bool {
        self.create_if_missing.is_some()
            || self.read_only.is_some()
            || self.journal_mode.is_some()
            || self.synchronous.is_some()
            || self.busy_timeout.is_some()
            || self.foreign_keys.is_some()
    }
}

/// SQLite journal mode
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
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

/// SQLite synchronous mode
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SynchronousMode {
    #[default]
    Full,
    Normal,
    Off,
}

impl SynchronousMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            SynchronousMode::Full => "Full",
            SynchronousMode::Normal => "Normal",
            SynchronousMode::Off => "Off",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContextField, Manifest};

    fn parse(content: &str) -> Manifest {
        toml::from_str(content).expect("Failed to parse TOML")
    }

    #[test]
    fn test_sqlite_basic() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [context.database]
            type = "sqlite"
            env = "DATABASE_URL"
            "#,
        );

        let database = schema.context.database.as_ref().unwrap();
        assert!(matches!(database, ContextField::Sqlite(_)));
        assert_eq!(database.rust_type(), "sqlx::SqlitePool");
    }

    #[test]
    fn test_sqlite_full_config() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [context.database]
            type = "sqlite"
            env = "DATABASE_URL"
            create_if_missing = true
            read_only = false
            journal_mode = "wal"
            synchronous = "normal"
            busy_timeout = 10000
            foreign_keys = true
            max_connections = 5
            "#,
        );

        let database = schema.context.database.as_ref().unwrap();
        let sqlite = database.sqlite_config().unwrap();

        assert!(sqlite.has_sqlite_options());
        assert_eq!(sqlite.create_if_missing, Some(true));
        assert_eq!(sqlite.read_only, Some(false));
        assert_eq!(sqlite.journal_mode, Some(JournalMode::Wal));
        assert_eq!(sqlite.synchronous, Some(SynchronousMode::Normal));
        assert_eq!(sqlite.busy_timeout, Some(10000));
        assert_eq!(sqlite.foreign_keys, Some(true));
        assert_eq!(sqlite.pool.max_connections, Some(5));
    }

    #[test]
    fn test_sqlite_journal_modes() {
        for (mode_str, expected) in [
            ("wal", JournalMode::Wal),
            ("delete", JournalMode::Delete),
            ("truncate", JournalMode::Truncate),
            ("persist", JournalMode::Persist),
            ("memory", JournalMode::Memory),
            ("off", JournalMode::Off),
        ] {
            let schema = parse(&format!(
                r#"
                [cli]
                name = "test"

                [context.database]
                type = "sqlite"
                journal_mode = "{}"
                "#,
                mode_str
            ));

            let database = schema.context.database.as_ref().unwrap();
            let sqlite = database.sqlite_config().unwrap();
            assert_eq!(sqlite.journal_mode, Some(expected));
        }
    }

    #[test]
    fn test_sqlite_synchronous_modes() {
        for (mode_str, expected) in [
            ("full", SynchronousMode::Full),
            ("normal", SynchronousMode::Normal),
            ("off", SynchronousMode::Off),
        ] {
            let schema = parse(&format!(
                r#"
                [cli]
                name = "test"

                [context.database]
                type = "sqlite"
                synchronous = "{}"
                "#,
                mode_str
            ));

            let database = schema.context.database.as_ref().unwrap();
            let sqlite = database.sqlite_config().unwrap();
            assert_eq!(sqlite.synchronous, Some(expected));
        }
    }

    #[test]
    fn test_sqlite_read_only() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [context.database]
            type = "sqlite"
            read_only = true
            "#,
        );

        let database = schema.context.database.as_ref().unwrap();
        let sqlite = database.sqlite_config().unwrap();
        assert_eq!(sqlite.read_only, Some(true));
    }

    #[test]
    fn test_sqlite_with_path() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [context.database]
            type = "sqlite"
            path = "db.sqlite"
            create_if_missing = true
            "#,
        );

        let database = schema.context.database.as_ref().unwrap();
        let sqlite = database.sqlite_config().unwrap();
        assert_eq!(sqlite.path, Some("db.sqlite".to_string()));
        assert_eq!(sqlite.create_if_missing, Some(true));
        // env should be None when path is used
        assert!(sqlite.env.is_none());
    }

    #[test]
    fn test_sqlite_path_with_options() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [context.database]
            type = "sqlite"
            path = "data/app.db"
            journal_mode = "wal"
            foreign_keys = true
            max_connections = 3
            "#,
        );

        let database = schema.context.database.as_ref().unwrap();
        let sqlite = database.sqlite_config().unwrap();
        assert_eq!(sqlite.path, Some("data/app.db".to_string()));
        assert_eq!(sqlite.journal_mode, Some(JournalMode::Wal));
        assert_eq!(sqlite.foreign_keys, Some(true));
        assert_eq!(sqlite.pool.max_connections, Some(3));
    }
}
