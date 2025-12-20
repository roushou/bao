use std::path::{Path, PathBuf};

use baobao_codegen::{
    language::TypeMapper,
    schema::{ContextFieldInfo, PoolConfigInfo},
};
use baobao_core::{ContextFieldType, DatabaseType, FileRules, GeneratedFile};

use super::GENERATED_HEADER;
use crate::{Field, Fn, Impl, MethodChain, RustFile, RustTypeMapper, Struct, Use};
const TYPE_MAPPER: RustTypeMapper = RustTypeMapper;

/// The context.rs file containing shared application state
pub struct ContextRs {
    pub fields: Vec<ContextFieldInfo>,
}

impl ContextRs {
    pub fn new(fields: Vec<ContextFieldInfo>) -> Self {
        Self { fields }
    }

    fn build_struct(&self) -> Struct {
        let mut s =
            Struct::new("Context").doc("Application context shared across all command handlers.");

        for field in &self.fields {
            let rust_type = TYPE_MAPPER.map_context_type(&field.field_type);
            s = s.field(Field::new(&field.name, rust_type));
        }

        s
    }

    fn build_impl(&self) -> Impl {
        let has_async = self.fields.iter().any(|f| f.is_async);

        let body = if self.fields.is_empty() {
            "Ok(Self {})".to_string()
        } else {
            let field_inits = self
                .fields
                .iter()
                .map(|f| format!("{}: {},", f.name, generate_init_code(f)))
                .collect::<Vec<_>>()
                .join("\n    ");
            format!("Ok(Self {{\n    {}\n}})", field_inits)
        };

        let new_fn = Fn::new("new")
            .returns("eyre::Result<Self>")
            .body(body)
            .async_if(has_async);

        Impl::new("Context").method(new_fn)
    }
}

impl GeneratedFile for ContextRs {
    fn path(&self, base: &Path) -> PathBuf {
        base.join("src").join("context.rs")
    }

    fn rules(&self) -> FileRules {
        FileRules::always_overwrite().with_header(GENERATED_HEADER)
    }

    fn render(&self) -> String {
        // Check if we need FromStr import (for SqliteConnectOptions::from_str)
        let needs_from_str = self.fields.iter().any(|f| {
            matches!(
                f.field_type,
                ContextFieldType::Database(DatabaseType::Sqlite)
            ) && (f.sqlite.as_ref().is_some_and(|s| s.has_config()) || f.pool.has_config())
        });

        let mut file = RustFile::new();

        if needs_from_str {
            file = file.use_stmt(Use::new("std::str").symbol("FromStr"));
        }

        file.add(self.build_struct())
            .add(self.build_impl())
            .render_with_header(GENERATED_HEADER)
    }
}

fn generate_init_code(field: &ContextFieldInfo) -> String {
    match field.field_type {
        ContextFieldType::Database(DatabaseType::Postgres | DatabaseType::Mysql) => {
            generate_sqlx_pool_init(field)
        }
        ContextFieldType::Database(DatabaseType::Sqlite) => generate_sqlite_init(field),
        ContextFieldType::Http => "reqwest::Client::new()".to_string(),
    }
}

fn generate_sqlx_pool_init(field: &ContextFieldInfo) -> String {
    let rust_type = TYPE_MAPPER.map_context_type(&field.field_type);
    if !field.pool.has_config() {
        return format!(
            "{}::connect(&std::env::var(\"{}\")?).await?",
            rust_type, field.env_var
        );
    }

    build_pool_options(&field.pool)
        .method_arg("connect", format!("&std::env::var(\"{}\")?", field.env_var))
        .await_()
        .try_()
        .build()
}

fn generate_sqlite_init(field: &ContextFieldInfo) -> String {
    let has_path = field.sqlite.as_ref().is_some_and(|s| s.path.is_some());
    let has_sqlite_opts = field.sqlite.as_ref().is_some_and(|s| s.has_config());
    let has_pool_opts = field.pool.has_config();

    // Simple case: no options, just connect
    if !has_path && !has_sqlite_opts && !has_pool_opts {
        return format!(
            "sqlx::SqlitePool::connect(&std::env::var(\"{}\")?).await?",
            field.env_var
        );
    }

    // Build SQLite connection options
    let options_chain = build_sqlite_options(field, has_path);
    let pool_chain = build_pool_options(&field.pool)
        .method_arg("connect_with", "options")
        .await_()
        .try_();

    format!(
        "{{\n            let options = {};\n            {}\n        }}",
        options_chain.build(),
        pool_chain.build()
    )
}

fn build_pool_options(pool: &PoolConfigInfo) -> MethodChain {
    MethodChain::new("sqlx::pool::PoolOptions::new()")
        .method_arg_opt("max_connections", pool.max_connections)
        .method_arg_opt("min_connections", pool.min_connections)
        .method_arg_opt(
            "acquire_timeout",
            pool.acquire_timeout
                .map(|t| format!("std::time::Duration::from_secs({})", t)),
        )
        .method_arg_opt(
            "idle_timeout",
            pool.idle_timeout
                .map(|t| format!("std::time::Duration::from_secs({})", t)),
        )
        .method_arg_opt(
            "max_lifetime",
            pool.max_lifetime
                .map(|t| format!("std::time::Duration::from_secs({})", t)),
        )
}

fn build_sqlite_options(field: &ContextFieldInfo, has_path: bool) -> MethodChain {
    let base = if has_path {
        let path = field.sqlite.as_ref().unwrap().path.as_ref().unwrap();
        MethodChain::new("sqlx::sqlite::SqliteConnectOptions::new()")
            .method_arg("filename", format!("\"{}\"", path))
    } else {
        MethodChain::new(format!(
            "sqlx::sqlite::SqliteConnectOptions::from_str(&std::env::var(\"{}\")?)?",
            field.env_var
        ))
    };

    let sqlite = field.sqlite.as_ref();

    base.method_arg_opt(
        "create_if_missing",
        sqlite.and_then(|s| s.create_if_missing),
    )
    .method_arg_opt("read_only", sqlite.and_then(|s| s.read_only))
    .method_arg_opt(
        "journal_mode",
        sqlite
            .and_then(|s| s.journal_mode.as_ref())
            .map(|m| format!("sqlx::sqlite::SqliteJournalMode::{}", m)),
    )
    .method_arg_opt(
        "synchronous",
        sqlite
            .and_then(|s| s.synchronous.as_ref())
            .map(|s| format!("sqlx::sqlite::SqliteSynchronous::{}", s)),
    )
    .method_arg_opt(
        "busy_timeout",
        sqlite
            .and_then(|s| s.busy_timeout)
            .map(|t| format!("std::time::Duration::from_millis({})", t)),
    )
    .method_arg_opt("foreign_keys", sqlite.and_then(|s| s.foreign_keys))
}
