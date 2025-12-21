//! SQLx database adapter.

use baobao_codegen::{
    adapters::{DatabaseAdapter, Dependency, ImportSpec, PoolConfig, PoolInitInfo, SqliteOptions},
    builder::{Block, BuilderSpec, Constructor, Value},
};
use baobao_ir::DatabaseType;

/// SQLx adapter for database pool generation.
#[derive(Debug, Clone, Default)]
pub struct SqlxAdapter;

impl SqlxAdapter {
    pub fn new() -> Self {
        Self
    }

    /// Build pool options spec from config.
    fn pool_options_spec(&self, pool: &PoolConfig) -> BuilderSpec {
        BuilderSpec::new("sqlx::pool::PoolOptions").apply_config([
            (
                "max_connections",
                pool.max_connections.map(|v| Value::uint(v.into())),
            ),
            (
                "min_connections",
                pool.min_connections.map(|v| Value::uint(v.into())),
            ),
            (
                "acquire_timeout",
                pool.acquire_timeout
                    .map(|d| Value::duration_secs(d.as_secs())),
            ),
            (
                "idle_timeout",
                pool.idle_timeout.map(|d| Value::duration_secs(d.as_secs())),
            ),
            (
                "max_lifetime",
                pool.max_lifetime.map(|d| Value::duration_secs(d.as_secs())),
            ),
        ])
    }

    /// Build SQLite connection options spec from config.
    fn sqlite_options_spec(&self, sqlite: &SqliteOptions, env_var: &str) -> BuilderSpec {
        let base = if let Some(path) = &sqlite.path {
            BuilderSpec::new("sqlx::sqlite::SqliteConnectOptions")
                .call_arg("filename", Value::string(path))
        } else {
            // Use from_str with env var
            BuilderSpec::with_constructor(Constructor::static_method(
                "sqlx::sqlite::SqliteConnectOptions",
                "from_str",
                vec![Value::env_var(env_var)],
            ))
            .try_()
        };

        base.apply_config([
            (
                "create_if_missing",
                sqlite.create_if_missing.map(Value::bool),
            ),
            ("read_only", sqlite.read_only.map(Value::bool)),
            (
                "journal_mode",
                sqlite
                    .journal_mode
                    .map(|m| Value::enum_variant("sqlx::sqlite::SqliteJournalMode", m.as_str())),
            ),
            (
                "synchronous",
                sqlite
                    .synchronous
                    .map(|s| Value::enum_variant("sqlx::sqlite::SqliteSynchronous", s.as_str())),
            ),
            (
                "busy_timeout",
                sqlite
                    .busy_timeout
                    .map(|d| Value::duration_millis(d.as_millis() as u64)),
            ),
            ("foreign_keys", sqlite.foreign_keys.map(Value::bool)),
        ])
    }

    /// Generate initialization for Postgres/MySQL pools.
    fn pool_init_simple(&self, info: &PoolInitInfo) -> Value {
        let pool_type = self.pool_type(info.db_type);

        if !info.pool_config.has_config() {
            // Simple case: Pool::connect(env_var).await?
            Value::builder(
                BuilderSpec::with_constructor(Constructor::static_new(pool_type))
                    .call_arg("connect", Value::env_var(&info.env_var))
                    .async_()
                    .try_(),
            )
        } else {
            // With pool options
            Value::builder(
                self.pool_options_spec(&info.pool_config)
                    .call_arg("connect", Value::env_var(&info.env_var))
                    .async_()
                    .try_(),
            )
        }
    }

    /// Generate initialization for SQLite pools.
    fn pool_init_sqlite(&self, info: &PoolInitInfo) -> Value {
        let has_path = info
            .sqlite_config
            .as_ref()
            .is_some_and(|s| s.path.is_some());
        let has_sqlite_opts = info.sqlite_config.as_ref().is_some_and(|s| s.has_config());
        let has_pool_opts = info.pool_config.has_config();

        // Simple case: no options, just connect
        if !has_path && !has_sqlite_opts && !has_pool_opts {
            return Value::builder(
                BuilderSpec::with_constructor(Constructor::static_new("sqlx::SqlitePool"))
                    .call_arg("connect", Value::env_var(&info.env_var))
                    .async_()
                    .try_(),
            );
        }

        // Build connection options
        let options_spec = match &info.sqlite_config {
            Some(sqlite) => self.sqlite_options_spec(sqlite, &info.env_var),
            None => BuilderSpec::with_constructor(Constructor::static_method(
                "sqlx::sqlite::SqliteConnectOptions",
                "from_str",
                vec![Value::env_var(&info.env_var)],
            ))
            .try_(),
        };

        // Build pool with connect_with
        let pool_spec = self
            .pool_options_spec(&info.pool_config)
            .call_arg("connect_with", Value::ident("options"))
            .async_()
            .try_();

        // Wrap in a block with let binding
        Value::block(
            Block::new(Value::builder(pool_spec)).binding("options", Value::builder(options_spec)),
        )
    }
}

impl DatabaseAdapter for SqlxAdapter {
    fn name(&self) -> &'static str {
        "sqlx"
    }

    fn dependencies(&self, db_type: DatabaseType) -> Vec<Dependency> {
        let features = match db_type {
            DatabaseType::Postgres => {
                r#"{ version = "0.8", features = ["runtime-tokio", "postgres"] }"#
            }
            DatabaseType::Mysql => r#"{ version = "0.8", features = ["runtime-tokio", "mysql"] }"#,
            DatabaseType::Sqlite => {
                r#"{ version = "0.8", features = ["runtime-tokio", "sqlite"] }"#
            }
        };
        vec![Dependency::new("sqlx", features)]
    }

    fn pool_type(&self, db_type: DatabaseType) -> &'static str {
        match db_type {
            DatabaseType::Postgres => "sqlx::PgPool",
            DatabaseType::Mysql => "sqlx::MySqlPool",
            DatabaseType::Sqlite => "sqlx::SqlitePool",
        }
    }

    fn pool_init(&self, info: &PoolInitInfo) -> Value {
        match info.db_type {
            DatabaseType::Postgres | DatabaseType::Mysql => self.pool_init_simple(info),
            DatabaseType::Sqlite => self.pool_init_sqlite(info),
        }
    }

    fn imports(&self, db_type: DatabaseType) -> Vec<ImportSpec> {
        match db_type {
            DatabaseType::Sqlite => {
                vec![
                    ImportSpec::new("sqlx").symbol("SqlitePool"),
                    ImportSpec::new("std::str").symbol("FromStr"),
                ]
            }
            DatabaseType::Postgres => vec![ImportSpec::new("sqlx").symbol("PgPool")],
            DatabaseType::Mysql => vec![ImportSpec::new("sqlx").symbol("MySqlPool")],
        }
    }

    fn requires_async(&self, _db_type: DatabaseType) -> bool {
        true
    }
}
