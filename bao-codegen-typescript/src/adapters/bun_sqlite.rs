//! Bun SQLite database adapter.

use baobao_codegen::{
    adapters::{DatabaseAdapter, Dependency, ImportSpec, PoolInitInfo},
    builder::Value,
};
use baobao_ir::DatabaseType;

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

    fn pool_init(&self, info: &PoolInitInfo) -> Value {
        match info.db_type {
            DatabaseType::Sqlite => {
                // For TypeScript, we use ident to generate `new Database(path)` pattern
                // The path is either a literal string or process.env.VAR
                let db_path = info
                    .sqlite_config
                    .as_ref()
                    .and_then(|c| c.path.as_ref())
                    .map(|p| format!("\"{}\"", p))
                    .unwrap_or_else(|| format!("process.env.{} ?? \":memory:\"", info.env_var));

                // Return as raw expression for now - TypeScript renderer will handle it
                Value::ident(format!("new Database({})", db_path))
            }
            _ => {
                // Other databases not yet supported in Bun natively
                Value::ident(format!("undefined /* {:?} not supported */", info.db_type))
            }
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
