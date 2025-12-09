pub mod mysql;
pub mod postgres;
pub mod sqlite;

use serde::Deserialize;

/// Trait for database configuration types.
///
/// This trait provides a common interface for accessing shared properties
/// across PostgreSQL, MySQL, and SQLite configurations, reducing code
/// duplication in `ContextField` methods.
pub trait DatabaseConfig {
    /// Get the environment variable name for the connection string.
    fn env(&self) -> Option<&str>;

    /// Get the pool configuration.
    fn pool(&self) -> &PoolConfig;

    /// Get the Rust type for this database pool.
    fn rust_type(&self) -> &'static str;

    /// Get the sqlx feature name for Cargo.toml.
    fn sqlx_feature(&self) -> &'static str;

    /// Get the default environment variable name.
    fn default_env(&self) -> &'static str {
        "DATABASE_URL"
    }

    /// Get the cargo dependencies needed for this database type.
    fn dependencies(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (
                "sqlx",
                match self.sqlx_feature() {
                    "postgres" => {
                        r#"{ version = "0.8", features = ["runtime-tokio", "postgres"] }"#
                    }
                    "mysql" => r#"{ version = "0.8", features = ["runtime-tokio", "mysql"] }"#,
                    "sqlite" => r#"{ version = "0.8", features = ["runtime-tokio", "sqlite"] }"#,
                    _ => r#"{ version = "0.8", features = ["runtime-tokio"] }"#,
                },
            ),
            (
                "tokio",
                r#"{ version = "1", features = ["rt-multi-thread", "macros"] }"#,
            ),
        ]
    }
}

impl DatabaseConfig for postgres::PostgresConfig {
    fn env(&self) -> Option<&str> {
        self.env.as_deref()
    }

    fn pool(&self) -> &PoolConfig {
        &self.pool
    }

    fn rust_type(&self) -> &'static str {
        "sqlx::PgPool"
    }

    fn sqlx_feature(&self) -> &'static str {
        "postgres"
    }
}

impl DatabaseConfig for mysql::MySqlConfig {
    fn env(&self) -> Option<&str> {
        self.env.as_deref()
    }

    fn pool(&self) -> &PoolConfig {
        &self.pool
    }

    fn rust_type(&self) -> &'static str {
        "sqlx::MySqlPool"
    }

    fn sqlx_feature(&self) -> &'static str {
        "mysql"
    }
}

impl DatabaseConfig for sqlite::SqliteConfig {
    fn env(&self) -> Option<&str> {
        self.env.as_deref()
    }

    fn pool(&self) -> &PoolConfig {
        &self.pool
    }

    fn rust_type(&self) -> &'static str {
        "sqlx::SqlitePool"
    }

    fn sqlx_feature(&self) -> &'static str {
        "sqlite"
    }
}

/// Database connection pool configuration
#[derive(Debug, Deserialize, Clone, Default)]
pub struct PoolConfig {
    /// Maximum number of connections in the pool (default: 10)
    pub max_connections: Option<u32>,

    /// Minimum number of connections to maintain (default: 0)
    pub min_connections: Option<u32>,

    /// Timeout for acquiring a connection from the pool, in seconds (default: 30)
    pub acquire_timeout: Option<u64>,

    /// Maximum time a connection can remain idle before being closed, in seconds (default: 600)
    pub idle_timeout: Option<u64>,

    /// Maximum lifetime of a connection, in seconds (default: 1800)
    pub max_lifetime: Option<u64>,
}

impl PoolConfig {
    /// Returns true if any pool option is configured
    pub fn has_config(&self) -> bool {
        self.max_connections.is_some()
            || self.min_connections.is_some()
            || self.acquire_timeout.is_some()
            || self.idle_timeout.is_some()
            || self.max_lifetime.is_some()
    }
}
