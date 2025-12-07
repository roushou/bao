mod database;
mod http;

pub use database::{
    mysql::MySqlConfig,
    postgres::PostgresConfig,
    sqlite::{JournalMode, SqliteConfig, SynchronousMode},
};
pub use http::HttpConfig;
use serde::Deserialize;

/// A context field declaration
#[derive(Debug, Clone)]
pub enum ContextField {
    /// PostgreSQL database pool
    Postgres(PostgresConfig),
    /// MySQL database pool
    Mysql(MySqlConfig),
    /// SQLite database pool
    Sqlite(SqliteConfig),
    /// HTTP client (only via [context.http])
    Http(HttpConfig),
}

/// Database context types (used for tagged deserialization)
#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub(crate) enum DatabaseContextField {
    Postgres(PostgresConfig),
    Mysql(MySqlConfig),
    Sqlite(SqliteConfig),
}

impl From<DatabaseContextField> for ContextField {
    fn from(db: DatabaseContextField) -> Self {
        match db {
            DatabaseContextField::Postgres(c) => ContextField::Postgres(c),
            DatabaseContextField::Mysql(c) => ContextField::Mysql(c),
            DatabaseContextField::Sqlite(c) => ContextField::Sqlite(c),
        }
    }
}

impl ContextField {
    /// Get the Rust type for this context field
    pub fn rust_type(&self) -> &'static str {
        match self {
            ContextField::Postgres(_) => "sqlx::PgPool",
            ContextField::Mysql(_) => "sqlx::MySqlPool",
            ContextField::Sqlite(_) => "sqlx::SqlitePool",
            ContextField::Http(_) => "reqwest::Client",
        }
    }

    /// Get the environment variable for this field
    pub fn env(&self) -> Option<&str> {
        match self {
            ContextField::Postgres(c) => c.env.as_deref(),
            ContextField::Mysql(c) => c.env.as_deref(),
            ContextField::Sqlite(c) => c.env.as_deref(),
            ContextField::Http(_) => None,
        }
    }

    /// Get the default environment variable name
    pub fn default_env(&self) -> &'static str {
        match self {
            ContextField::Postgres(_) | ContextField::Mysql(_) | ContextField::Sqlite(_) => {
                "DATABASE_URL"
            }
            ContextField::Http(_) => "",
        }
    }

    /// Get the cargo dependencies needed for this type
    pub fn dependencies(&self) -> Vec<(&'static str, &'static str)> {
        match self {
            ContextField::Postgres(_) => vec![
                (
                    "sqlx",
                    r#"{ version = "0.8", features = ["runtime-tokio", "postgres"] }"#,
                ),
                (
                    "tokio",
                    r#"{ version = "1", features = ["rt-multi-thread", "macros"] }"#,
                ),
            ],
            ContextField::Mysql(_) => vec![
                (
                    "sqlx",
                    r#"{ version = "0.8", features = ["runtime-tokio", "mysql"] }"#,
                ),
                (
                    "tokio",
                    r#"{ version = "1", features = ["rt-multi-thread", "macros"] }"#,
                ),
            ],
            ContextField::Sqlite(_) => vec![
                (
                    "sqlx",
                    r#"{ version = "0.8", features = ["runtime-tokio", "sqlite"] }"#,
                ),
                (
                    "tokio",
                    r#"{ version = "1", features = ["rt-multi-thread", "macros"] }"#,
                ),
            ],
            ContextField::Http(_) => {
                vec![("reqwest", r#"{ version = "0.12", features = ["json"] }"#)]
            }
        }
    }

    /// Returns true if this type requires async initialization
    pub fn is_async(&self) -> bool {
        matches!(
            self,
            ContextField::Postgres(_) | ContextField::Mysql(_) | ContextField::Sqlite(_)
        )
    }

    /// Returns true if this is a database type
    pub fn is_database(&self) -> bool {
        matches!(
            self,
            ContextField::Postgres(_) | ContextField::Mysql(_) | ContextField::Sqlite(_)
        )
    }

    /// Get pool configuration if this is a database type
    pub fn pool_config(&self) -> Option<&PoolConfig> {
        match self {
            ContextField::Postgres(c) => Some(&c.pool),
            ContextField::Mysql(c) => Some(&c.pool),
            ContextField::Sqlite(c) => Some(&c.pool),
            _ => None,
        }
    }

    /// Get SQLite-specific configuration
    pub fn sqlite_config(&self) -> Option<&SqliteConfig> {
        match self {
            ContextField::Sqlite(c) => Some(c),
            _ => None,
        }
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

/// Application context configuration
/// Only allows [context.database] and [context.http]
#[derive(Debug, Clone, Default)]
pub struct Context {
    /// Database connection pool (postgres, mysql, or sqlite)
    pub database: Option<ContextField>,
    /// HTTP client
    pub http: Option<HttpConfig>,
}

impl Context {
    /// Returns true if no context is configured
    pub fn is_empty(&self) -> bool {
        self.database.is_none() && self.http.is_none()
    }

    /// Returns the number of configured context fields
    pub fn len(&self) -> usize {
        let mut count = 0;
        if self.database.is_some() {
            count += 1;
        }
        if self.http.is_some() {
            count += 1;
        }
        count
    }

    /// Returns true if any async context is configured (database)
    pub fn has_async(&self) -> bool {
        self.database.is_some()
    }

    /// Check if a context field exists by name
    pub fn has_field(&self, name: &str) -> bool {
        match name {
            "database" => self.database.is_some(),
            "http" => self.http.is_some(),
            _ => false,
        }
    }

    /// Get all context fields as a vector of (name, field) pairs
    pub fn fields(&self) -> Vec<(&'static str, ContextField)> {
        let mut fields = Vec::new();
        if let Some(db) = &self.database {
            fields.push(("database", db.clone()));
        }
        if let Some(http) = &self.http {
            fields.push(("http", ContextField::Http(http.clone())));
        }
        fields
    }
}

#[cfg(test)]
mod tests {
    use crate::Schema;

    fn parse(content: &str) -> Schema {
        toml::from_str(content).expect("Failed to parse TOML")
    }

    #[test]
    fn test_context_multiple_fields() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [context.database]
            type = "postgres"
            env = "DATABASE_URL"

            [context.http]
            "#,
        );

        assert_eq!(schema.context.len(), 2);

        let database = schema.context.database.as_ref().unwrap();
        assert!(matches!(database, super::ContextField::Postgres(_)));
        assert!(database.is_async());

        let http = schema.context.http.as_ref().unwrap();
        assert_eq!(http.timeout, None);
    }

    #[test]
    fn test_context_database_types() {
        // Test postgres
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [context.database]
            type = "postgres"
            "#,
        );
        assert!(matches!(
            schema.context.database.as_ref().unwrap(),
            super::ContextField::Postgres(_)
        ));

        // Test mysql
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [context.database]
            type = "mysql"
            "#,
        );
        assert!(matches!(
            schema.context.database.as_ref().unwrap(),
            super::ContextField::Mysql(_)
        ));

        // Test sqlite
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [context.database]
            type = "sqlite"
            "#,
        );
        assert!(matches!(
            schema.context.database.as_ref().unwrap(),
            super::ContextField::Sqlite(_)
        ));
    }

    #[test]
    fn test_empty_context() {
        let schema = parse(
            r#"
            [cli]
            name = "test"
            "#,
        );

        assert!(schema.context.is_empty());
    }
}
