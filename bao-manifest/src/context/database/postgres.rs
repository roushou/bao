use serde::Deserialize;

use super::{BasicDbConfig, DatabaseConfig, PoolConfig};

/// Configuration for PostgreSQL database.
///
/// A newtype wrapper around [`BasicDbConfig`] that provides PostgreSQL-specific
/// trait implementations.
#[derive(Debug, Deserialize, Clone, Default)]
#[serde(transparent)]
pub struct PostgresConfig(pub BasicDbConfig);

impl PostgresConfig {
    /// Get the environment variable for connection string.
    pub fn env(&self) -> Option<&str> {
        self.0.env.as_deref()
    }

    /// Get the pool configuration.
    pub fn pool(&self) -> &PoolConfig {
        &self.0.pool
    }
}

impl DatabaseConfig for PostgresConfig {
    fn env(&self) -> Option<&str> {
        self.0.env.as_deref()
    }

    fn pool(&self) -> &PoolConfig {
        &self.0.pool
    }

    fn sqlx_feature(&self) -> &'static str {
        "postgres"
    }
}

#[cfg(test)]
mod tests {
    use crate::{ContextField, Manifest};

    fn parse(content: &str) -> Manifest {
        toml::from_str(content).expect("Failed to parse TOML")
    }

    #[test]
    fn test_context_postgres() {
        let schema = parse(
            r#"
            [cli]
            name = "test"
            language = "rust"

            [context.database]
            type = "postgres"
            env = "DATABASE_URL"
            "#,
        );

        let database = schema.context.database.as_ref().unwrap();
        assert!(matches!(database, ContextField::Postgres(_)));
        assert_eq!(database.env(), Some("DATABASE_URL"));
    }

    #[test]
    fn test_context_default_env() {
        let schema = parse(
            r#"
            [cli]
            name = "test"
            language = "rust"

            [context.database]
            type = "postgres"
            "#,
        );

        let database = schema.context.database.as_ref().unwrap();
        assert_eq!(database.env(), None);
        assert_eq!(database.default_env(), "DATABASE_URL");
    }

    #[test]
    fn test_pool_config_full() {
        let schema = parse(
            r#"
            [cli]
            name = "test"
            language = "rust"

            [context.database]
            type = "postgres"
            env = "DATABASE_URL"
            max_connections = 20
            min_connections = 5
            acquire_timeout = 60
            idle_timeout = 300
            max_lifetime = 3600
            "#,
        );

        let database = schema.context.database.as_ref().unwrap();
        let pool = database.pool_config().unwrap();
        assert!(pool.has_config());
        assert_eq!(pool.max_connections, Some(20));
        assert_eq!(pool.min_connections, Some(5));
        assert_eq!(pool.acquire_timeout, Some(60));
        assert_eq!(pool.idle_timeout, Some(300));
        assert_eq!(pool.max_lifetime, Some(3600));
    }

    #[test]
    fn test_pool_config_partial() {
        let schema = parse(
            r#"
            [cli]
            name = "test"
            language = "rust"

            [context.database]
            type = "postgres"
            max_connections = 10
            "#,
        );

        let database = schema.context.database.as_ref().unwrap();
        let pool = database.pool_config().unwrap();
        assert!(pool.has_config());
        assert_eq!(pool.max_connections, Some(10));
        assert_eq!(pool.min_connections, None);
    }

    #[test]
    fn test_pool_config_default() {
        let schema = parse(
            r#"
            [cli]
            name = "test"
            language = "rust"

            [context.database]
            type = "postgres"
            "#,
        );

        let database = schema.context.database.as_ref().unwrap();
        let pool = database.pool_config().unwrap();
        assert!(!pool.has_config());
    }

    #[test]
    fn test_pool_config_inline_format() {
        let schema = parse(
            r#"
            [cli]
            name = "test"
            language = "rust"

            [context]
            database = { type = "postgres", max_connections = 15, min_connections = 2 }
            "#,
        );

        let database = schema.context.database.as_ref().unwrap();
        let pool = database.pool_config().unwrap();
        assert!(pool.has_config());
        assert_eq!(pool.max_connections, Some(15));
        assert_eq!(pool.min_connections, Some(2));
    }
}
