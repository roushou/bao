//! SQLx database adapter.

use baobao_codegen::{
    adapters::{
        DatabaseAdapter, DatabaseOptionsInfo, Dependency, ImportSpec, PoolConfig, PoolInitInfo,
    },
    builder::CodeFragment,
};
use baobao_core::DatabaseType;

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

        let mut code = String::from("sqlx::sqlite::SqliteConnectOptions::new()");

        if let Some(create) = sqlite.create_if_missing {
            code.push_str(&format!("\n    .create_if_missing({})", create));
        }
        if let Some(read_only) = sqlite.read_only {
            code.push_str(&format!("\n    .read_only({})", read_only));
        }
        if let Some(ref journal_mode) = sqlite.journal_mode {
            code.push_str(&format!(
                "\n    .journal_mode(sqlx::sqlite::SqliteJournalMode::{})",
                journal_mode
            ));
        }
        if let Some(ref synchronous) = sqlite.synchronous {
            code.push_str(&format!(
                "\n    .synchronous(sqlx::sqlite::SqliteSynchronous::{})",
                synchronous
            ));
        }
        if let Some(busy_timeout) = sqlite.busy_timeout {
            code.push_str(&format!(
                "\n    .busy_timeout(std::time::Duration::from_millis({}))",
                busy_timeout
            ));
        }
        if let Some(foreign_keys) = sqlite.foreign_keys {
            code.push_str(&format!("\n    .foreign_keys({})", foreign_keys));
        }

        Some(vec![CodeFragment::raw(code)])
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

    let mut code = String::from("sqlx::pool::PoolOptions::new()\n                ");
    append_pool_options(&mut code, &info.pool_config);
    code.push_str(&format!(
        ".connect(&std::env::var(\"{}\")?).await?",
        info.env_var
    ));
    code
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

    let mut code = String::new();

    // Build connection options - use path directly or from env var
    if has_path {
        let path = info.sqlite_config.as_ref().unwrap().path.as_ref().unwrap();
        code.push_str(&format!(
            "{{\n            let options = sqlx::sqlite::SqliteConnectOptions::new()\n                .filename(\"{}\")",
            path
        ));
    } else {
        code.push_str(&format!(
            "{{\n            let options = sqlx::sqlite::SqliteConnectOptions::from_str(&std::env::var(\"{}\")?)?",
            info.env_var
        ));
    }

    if let Some(sqlite) = &info.sqlite_config {
        if let Some(create) = sqlite.create_if_missing {
            code.push_str(&format!("\n                .create_if_missing({})", create));
        }
        if let Some(read_only) = sqlite.read_only {
            code.push_str(&format!("\n                .read_only({})", read_only));
        }
        if let Some(ref journal_mode) = sqlite.journal_mode {
            code.push_str(&format!(
                "\n                .journal_mode(sqlx::sqlite::SqliteJournalMode::{})",
                journal_mode
            ));
        }
        if let Some(ref synchronous) = sqlite.synchronous {
            code.push_str(&format!(
                "\n                .synchronous(sqlx::sqlite::SqliteSynchronous::{})",
                synchronous
            ));
        }
        if let Some(busy_timeout) = sqlite.busy_timeout {
            code.push_str(&format!(
                "\n                .busy_timeout(std::time::Duration::from_millis({}))",
                busy_timeout
            ));
        }
        if let Some(foreign_keys) = sqlite.foreign_keys {
            code.push_str(&format!(
                "\n                .foreign_keys({})",
                foreign_keys
            ));
        }
    }

    code.push_str(";\n            ");

    // Build pool options
    code.push_str("sqlx::pool::PoolOptions::new()\n                ");
    append_pool_options(&mut code, &info.pool_config);
    code.push_str(".connect_with(options).await?\n        }");

    code
}

fn append_pool_options(code: &mut String, pool: &PoolConfig) {
    if let Some(max) = pool.max_connections {
        code.push_str(&format!(".max_connections({})\n                ", max));
    }
    if let Some(min) = pool.min_connections {
        code.push_str(&format!(".min_connections({})\n                ", min));
    }
    if let Some(timeout) = pool.acquire_timeout {
        code.push_str(&format!(
            ".acquire_timeout(std::time::Duration::from_secs({}))\n                ",
            timeout
        ));
    }
    if let Some(timeout) = pool.idle_timeout {
        code.push_str(&format!(
            ".idle_timeout(std::time::Duration::from_secs({}))\n                ",
            timeout
        ));
    }
    if let Some(lifetime) = pool.max_lifetime {
        code.push_str(&format!(
            ".max_lifetime(std::time::Duration::from_secs({}))\n                ",
            lifetime
        ));
    }
}
