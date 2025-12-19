//! Bun SQLite database adapter.

use baobao_codegen::{
    adapters::{DatabaseAdapter, DatabaseOptionsInfo, Dependency, ImportSpec, PoolInitInfo},
    builder::CodeFragment,
};
use baobao_core::DatabaseType;

use crate::ast::JsObject;

/// Bun SQLite adapter using bun:sqlite.
#[derive(Debug, Clone, Default)]
pub struct BunSqliteAdapter;

impl BunSqliteAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl DatabaseAdapter for BunSqliteAdapter {
    fn name(&self) -> &'static str {
        "bun:sqlite"
    }

    fn dependencies(&self, _db_type: DatabaseType) -> Vec<Dependency> {
        // bun:sqlite is built into Bun, no external dependencies needed
        Vec::new()
    }

    fn pool_type(&self, db_type: DatabaseType) -> &'static str {
        match db_type {
            DatabaseType::Sqlite => "Database",
            // Bun currently only has native SQLite support
            DatabaseType::Postgres | DatabaseType::Mysql => "unknown",
        }
    }

    fn generate_pool_init(&self, info: &PoolInitInfo) -> Vec<CodeFragment> {
        match info.db_type {
            DatabaseType::Sqlite => {
                let db_path = info
                    .sqlite_config
                    .as_ref()
                    .and_then(|c| c.path.as_ref())
                    .map(|p| format!("\"{}\"", p))
                    .unwrap_or_else(|| format!("process.env.{} ?? \":memory:\"", info.env_var));

                let code = format!("const {} = new Database({});", info.field_name, db_path);

                vec![CodeFragment::raw(code)]
            }
            _ => {
                // Other databases not yet supported in Bun natively
                vec![CodeFragment::raw(format!(
                    "// TODO: {:?} database not yet supported",
                    info.db_type
                ))]
            }
        }
    }

    fn generate_options(&self, info: &DatabaseOptionsInfo) -> Option<Vec<CodeFragment>> {
        match info.db_type {
            DatabaseType::Sqlite => {
                if let Some(sqlite) = &info.sqlite {
                    let opts = JsObject::new()
                        .raw_if(sqlite.read_only == Some(true), "readonly", "true")
                        .raw_if(sqlite.create_if_missing == Some(true), "create", "true");

                    if !opts.is_empty() {
                        return Some(vec![CodeFragment::raw(opts.build())]);
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn imports(&self, db_type: DatabaseType) -> Vec<ImportSpec> {
        match db_type {
            DatabaseType::Sqlite => {
                vec![ImportSpec::new("bun:sqlite").symbol("Database")]
            }
            _ => Vec::new(),
        }
    }

    fn requires_async(&self, _db_type: DatabaseType) -> bool {
        // bun:sqlite is synchronous
        false
    }
}
