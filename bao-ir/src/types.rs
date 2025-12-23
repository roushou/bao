//! Core type definitions.

use crate::{PoolConfig, SqliteOptions};

/// Database type for context fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DatabaseType {
    Postgres,
    Mysql,
    Sqlite,
}

impl DatabaseType {
    /// Get the lowercase string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            DatabaseType::Postgres => "postgres",
            DatabaseType::Mysql => "mysql",
            DatabaseType::Sqlite => "sqlite",
        }
    }
}

/// Context field type - language-agnostic representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextFieldType {
    /// Database connection pool.
    Database(DatabaseType),
    /// HTTP client.
    Http,
}

impl ContextFieldType {
    /// Returns true if this field type requires async initialization.
    pub fn is_async(&self) -> bool {
        matches!(self, ContextFieldType::Database(_))
    }
}

/// Info about a context field for code generation.
#[derive(Debug, Clone)]
pub struct ContextFieldInfo {
    /// Field name in the context struct.
    pub name: String,
    /// Language-agnostic field type.
    pub field_type: ContextFieldType,
    /// Environment variable for configuration.
    pub env_var: String,
    /// Whether initialization is async.
    pub is_async: bool,
    /// Connection pool configuration.
    pub pool: PoolConfig,
    /// SQLite-specific options.
    pub sqlite: Option<SqliteOptions>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_type_as_str() {
        assert_eq!(DatabaseType::Postgres.as_str(), "postgres");
        assert_eq!(DatabaseType::Mysql.as_str(), "mysql");
        assert_eq!(DatabaseType::Sqlite.as_str(), "sqlite");
    }

    #[test]
    fn test_context_field_type_is_async() {
        assert!(ContextFieldType::Database(DatabaseType::Postgres).is_async());
        assert!(ContextFieldType::Database(DatabaseType::Mysql).is_async());
        assert!(ContextFieldType::Database(DatabaseType::Sqlite).is_async());
        assert!(!ContextFieldType::Http.is_async());
    }
}
