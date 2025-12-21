//! SQLx database adapter.

use baobao_codegen::{
    adapters::{
        DatabaseAdapter, DatabaseOptionsInfo, Dependency, ImportSpec, PoolConfig, PoolInitInfo,
        SqliteConfig,
    },
    builder::{Block, BuilderSpec, CodeFragment, Constructor, RenderExt, RenderOptions, Value},
};
use baobao_core::DatabaseType;

use crate::RustRenderer;

/// SQLx adapter for database pool generation.
#[derive(Debug, Clone, Default)]
pub struct SqlxAdapter;

impl SqlxAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl DatabaseAdapter for SqlxAdapter {
    fn name(&self) -> &'static str {
        "sqlx"
    }

    fn dependencies(&self, db_type: DatabaseType) -> Vec<Dependency> {
        let runtime_features = match db_type {
            DatabaseType::Postgres => {
                r#"{ version = "0.8", features = ["runtime-tokio", "postgres"] }"#
            }
            DatabaseType::Mysql => r#"{ version = "0.8", features = ["runtime-tokio", "mysql"] }"#,
            DatabaseType::Sqlite => {
                r#"{ version = "0.8", features = ["runtime-tokio", "sqlite"] }"#
            }
        };
        vec![Dependency::new("sqlx", runtime_features)]
    }

    fn pool_type(&self, db_type: DatabaseType) -> &'static str {
        match db_type {
            DatabaseType::Postgres => "sqlx::PgPool",
            DatabaseType::Mysql => "sqlx::MySqlPool",
            DatabaseType::Sqlite => "sqlx::SqlitePool",
        }
    }

    fn generate_pool_init(&self, info: &PoolInitInfo) -> Vec<CodeFragment> {
        let code = match info.db_type {
            DatabaseType::Postgres | DatabaseType::Mysql => generate_sqlx_pool_init(info),
            DatabaseType::Sqlite => generate_sqlite_init(info),
        };

        vec![CodeFragment::raw(code)]
    }

    fn generate_options(&self, info: &DatabaseOptionsInfo) -> Option<Vec<CodeFragment>> {
        if info.db_type != DatabaseType::Sqlite {
            return None;
        }

        let sqlite = info.sqlite.as_ref()?;
        if !sqlite.has_config() {
            return None;
        }

        let spec = sqlite_connect_options_spec(sqlite);
        let opts = RenderOptions::default().with_indent(1);
        Some(vec![CodeFragment::raw(
            spec.render_with(&RustRenderer, &opts),
        )])
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

fn generate_sqlx_pool_init(info: &PoolInitInfo) -> String {
    let pool_type = match info.db_type {
        DatabaseType::Postgres => "sqlx::PgPool",
        DatabaseType::Mysql => "sqlx::MySqlPool",
        DatabaseType::Sqlite => "sqlx::SqlitePool",
    };

    if !info.pool_config.has_config() {
        return format!(
            "{}::connect(&std::env::var(\"{}\")?).await?",
            pool_type, info.env_var
        );
    }

    let spec = pool_options_spec(&info.pool_config)
        .call_arg("connect", env_var_expr(&info.env_var))
        .async_()
        .try_();

    spec.render_with(&RustRenderer, &RenderOptions::default().with_indent(3))
}

fn generate_sqlite_init(info: &PoolInitInfo) -> String {
    let has_path = info
        .sqlite_config
        .as_ref()
        .is_some_and(|s| s.path.is_some());
    let has_sqlite_opts = info.sqlite_config.as_ref().is_some_and(|s| s.has_config());
    let has_pool_opts = info.pool_config.has_config();

    // Simple case: no options, just connect
    if !has_path && !has_sqlite_opts && !has_pool_opts {
        return format!(
            "sqlx::SqlitePool::connect(&std::env::var(\"{}\")?).await?",
            info.env_var
        );
    }

    // Build connection options spec
    let options_spec = if has_path {
        let path = info.sqlite_config.as_ref().unwrap().path.as_ref().unwrap();
        let base = BuilderSpec::new("sqlx::sqlite::SqliteConnectOptions")
            .call_arg("filename", Value::string(path));
        match &info.sqlite_config {
            Some(s) => apply_sqlite_config(base, s),
            None => base,
        }
    } else {
        let base = BuilderSpec::with_constructor(Constructor::raw(format!(
            "sqlx::sqlite::SqliteConnectOptions::from_str(&std::env::var(\"{}\")?)?",
            info.env_var
        )));
        match &info.sqlite_config {
            Some(s) => apply_sqlite_config(base, s),
            None => base,
        }
    };

    // Build pool chain that uses the options
    let pool_spec = pool_options_spec(&info.pool_config)
        .call_arg("connect_with", Value::ident("options"))
        .async_()
        .try_();

    // Render as a block with let binding
    let block =
        Block::new(Value::builder(pool_spec)).binding("options", Value::builder(options_spec));

    block.render_with(&RustRenderer, &RenderOptions::default().with_indent(2))
}

/// Build SQLx pool options spec.
fn pool_options_spec(pool: &PoolConfig) -> BuilderSpec {
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
            pool.acquire_timeout.map(Value::duration_secs),
        ),
        ("idle_timeout", pool.idle_timeout.map(Value::duration_secs)),
        ("max_lifetime", pool.max_lifetime.map(Value::duration_secs)),
    ])
}

/// Build SQLite connection options spec.
fn sqlite_connect_options_spec(sqlite: &SqliteConfig) -> BuilderSpec {
    apply_sqlite_config(
        BuilderSpec::new("sqlx::sqlite::SqliteConnectOptions"),
        sqlite,
    )
}

/// Apply SQLite configuration to a builder spec.
fn apply_sqlite_config(spec: BuilderSpec, sqlite: &SqliteConfig) -> BuilderSpec {
    spec.apply_config([
        (
            "create_if_missing",
            sqlite.create_if_missing.map(Value::bool),
        ),
        ("read_only", sqlite.read_only.map(Value::bool)),
        (
            "journal_mode",
            sqlite
                .journal_mode
                .as_ref()
                .map(|m| Value::enum_variant("sqlx::sqlite::SqliteJournalMode", m)),
        ),
        (
            "synchronous",
            sqlite
                .synchronous
                .as_ref()
                .map(|s| Value::enum_variant("sqlx::sqlite::SqliteSynchronous", s)),
        ),
        (
            "busy_timeout",
            sqlite.busy_timeout.map(Value::duration_millis),
        ),
        ("foreign_keys", sqlite.foreign_keys.map(Value::bool)),
    ])
}

/// Create an expression for reading an environment variable.
fn env_var_expr(name: &str) -> Value {
    Value::ident(format!("&std::env::var(\"{}\")?", name))
}
