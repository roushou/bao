mod database;
mod http;

pub use database::{
    DatabaseConfig, PoolConfig,
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
    /// Get the database configuration if this is a database type.
    ///
    /// Returns `Some(&dyn DatabaseConfig)` for Postgres, MySQL, and SQLite,
    /// or `None` for HTTP.
    pub fn as_database(&self) -> Option<&dyn DatabaseConfig> {
        match self {
            ContextField::Postgres(c) => Some(c),
            ContextField::Mysql(c) => Some(c),
            ContextField::Sqlite(c) => Some(c),
            ContextField::Http(_) => None,
        }
    }

    /// Get the Rust type for this context field
    pub fn rust_type(&self) -> &'static str {
        match self.as_database() {
            Some(db) => db.rust_type(),
            None => "reqwest::Client",
        }
    }

    /// Get the environment variable for this field
    pub fn env(&self) -> Option<&str> {
        self.as_database().and_then(|db| db.env())
    }

    /// Get the default environment variable name
    pub fn default_env(&self) -> &'static str {
        match self.as_database() {
            Some(db) => db.default_env(),
            None => "",
        }
    }

    /// Get the cargo dependencies needed for this type
    pub fn dependencies(&self) -> Vec<(&'static str, &'static str)> {
        match self.as_database() {
            Some(db) => db.dependencies(),
            None => vec![("reqwest", r#"{ version = "0.12", features = ["json"] }"#)],
        }
    }

    /// Returns true if this type requires async initialization
    pub fn is_async(&self) -> bool {
        self.as_database().is_some()
    }

    /// Returns true if this is a database type
    pub fn is_database(&self) -> bool {
        self.as_database().is_some()
    }

    /// Get pool configuration if this is a database type
    pub fn pool_config(&self) -> Option<&PoolConfig> {
        self.as_database().map(|db| db.pool())
    }

    /// Get SQLite-specific configuration
    pub fn sqlite_config(&self) -> Option<&SqliteConfig> {
        match self {
            ContextField::Sqlite(c) => Some(c),
            _ => None,
        }
    }

    /// Get HTTP-specific configuration
    pub fn http_config(&self) -> Option<&HttpConfig> {
        match self {
            ContextField::Http(c) => Some(c),
            _ => None,
        }
    }
}

/// Application context configuration
/// Only allows [context.database] and [context.http]
#[derive(Debug, Clone, Default)]
pub struct Context {
    /// Database connection pool (postgres, mysql, or sqlite)
    pub database: Option<ContextField>,
    /// HTTP client (stored as ContextField for uniform iteration)
    pub http: Option<ContextField>,
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
    pub fn fields(&self) -> Vec<(&'static str, &ContextField)> {
        let mut fields = Vec::new();
        if let Some(db) = &self.database {
            fields.push(("database", db));
        }
        if let Some(http) = &self.http {
            fields.push(("http", http));
        }
        fields
    }

    /// Get the HTTP configuration if present
    pub fn http_config(&self) -> Option<&HttpConfig> {
        self.http.as_ref().and_then(|f| f.http_config())
    }
}

/// Custom deserializer for Context that handles database and http fields
pub(crate) fn deserialize<'de, D>(deserializer: D) -> std::result::Result<Context, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    #[derive(Deserialize)]
    struct RawContext {
        database: Option<toml::Value>,
        http: Option<toml::Value>,
    }

    let raw: RawContext = RawContext::deserialize(deserializer)?;
    let mut ctx = Context::default();

    if let Some(db_value) = raw.database {
        let db: DatabaseContextField = db_value
            .try_into()
            .map_err(|e: toml::de::Error| D::Error::custom(e.message()))?;
        ctx.database = Some(db.into());
    }

    if let Some(http_value) = raw.http {
        let http: HttpConfig = http_value
            .try_into()
            .map_err(|e: toml::de::Error| D::Error::custom(e.message()))?;
        ctx.http = Some(ContextField::Http(http));
    }

    Ok(ctx)
}

#[cfg(test)]
mod tests {
    use crate::Manifest;

    fn parse(content: &str) -> Manifest {
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

        let http = schema.context.http_config().unwrap();
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
