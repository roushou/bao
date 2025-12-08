use serde::Deserialize;

use crate::PoolConfig;

/// Configuration for PostgreSQL and MySQL databases
#[derive(Debug, Deserialize, Clone, Default)]
pub struct PostgresConfig {
    /// Environment variable for connection string
    pub env: Option<String>,

    /// Pool configuration
    #[serde(flatten)]
    pub pool: PoolConfig,
}

#[cfg(test)]
mod tests {
    use crate::{ContextField, Schema};

    fn parse(content: &str) -> Schema {
        toml::from_str(content).expect("Failed to parse TOML")
    }

    #[test]
    fn test_context_postgres() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

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
