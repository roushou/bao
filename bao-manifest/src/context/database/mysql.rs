use serde::Deserialize;

use super::{BasicDbConfig, DatabaseConfig, PoolConfig};

/// Configuration for MySQL database.
///
/// A newtype wrapper around [`BasicDbConfig`] that provides MySQL-specific
/// trait implementations.
#[derive(Debug, Deserialize, Clone, Default)]
#[serde(transparent)]
pub struct MySqlConfig(pub BasicDbConfig);

impl MySqlConfig {
    /// Get the environment variable for connection string.
    pub fn env(&self) -> Option<&str> {
        self.0.env.as_deref()
    }

    /// Get the pool configuration.
    pub fn pool(&self) -> &PoolConfig {
        &self.0.pool
    }
}

impl DatabaseConfig for MySqlConfig {
    fn env(&self) -> Option<&str> {
        self.0.env.as_deref()
    }

    fn pool(&self) -> &PoolConfig {
        &self.0.pool
    }

    fn sqlx_feature(&self) -> &'static str {
        "mysql"
    }
}

#[cfg(test)]
mod tests {
    use crate::{ContextField, Manifest};

    fn parse(content: &str) -> Manifest {
        toml::from_str(content).expect("Failed to parse TOML")
    }

    #[test]
    fn test_context_mysql() {
        let schema = parse(
            r#"
            [cli]
            name = "test"
            language = "rust"

            [context.database]
            type = "mysql"
            env = "DATABASE_URL"
            "#,
        );

        let database = schema.context.database.as_ref().unwrap();
        assert!(matches!(database, ContextField::Mysql(_)));
        assert_eq!(database.env(), Some("DATABASE_URL"));
        assert_eq!(database.type_name(), "mysql");
    }

    #[test]
    fn test_context_default_env() {
        let schema = parse(
            r#"
            [cli]
            name = "test"
            language = "rust"

            [context.database]
            type = "mysql"
            "#,
        );

        let database = schema.context.database.as_ref().unwrap();
        assert_eq!(database.env(), None);
        assert_eq!(database.default_env(), "DATABASE_URL");
    }

    #[test]
    fn test_pool_config() {
        let schema = parse(
            r#"
            [cli]
            name = "test"
            language = "rust"

            [context.database]
            type = "mysql"
            max_connections = 20
            "#,
        );

        let database = schema.context.database.as_ref().unwrap();
        let pool = database.pool_config().unwrap();
        assert!(pool.has_config());
        assert_eq!(pool.max_connections, Some(20));
    }
}
