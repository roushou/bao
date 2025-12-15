//! Types for code generation.

use baobao_core::{ContextFieldType, DatabaseType};
use baobao_manifest::{Context, ContextField};

/// Info about a command for code generation
#[derive(Debug, Clone)]
pub struct CommandInfo {
    pub name: String,
    pub description: String,
    pub has_subcommands: bool,
}

/// Info about a context field for code generation
#[derive(Debug, Clone)]
pub struct ContextFieldInfo {
    pub name: String,
    /// Language-agnostic field type
    pub field_type: ContextFieldType,
    pub env_var: String,
    pub is_async: bool,
    pub pool: PoolConfigInfo,
    pub sqlite: Option<SqliteConfigInfo>,
}

/// Pool configuration for code generation
#[derive(Debug, Clone, Default)]
pub struct PoolConfigInfo {
    pub max_connections: Option<u32>,
    pub min_connections: Option<u32>,
    pub acquire_timeout: Option<u64>,
    pub idle_timeout: Option<u64>,
    pub max_lifetime: Option<u64>,
}

impl PoolConfigInfo {
    pub fn has_config(&self) -> bool {
        self.max_connections.is_some()
            || self.min_connections.is_some()
            || self.acquire_timeout.is_some()
            || self.idle_timeout.is_some()
            || self.max_lifetime.is_some()
    }
}

/// SQLite-specific configuration for code generation
#[derive(Debug, Clone, Default)]
pub struct SqliteConfigInfo {
    /// Direct file path (e.g., "db.sqlite") - takes precedence over env var
    pub path: Option<String>,
    pub create_if_missing: Option<bool>,
    pub read_only: Option<bool>,
    pub journal_mode: Option<String>,
    pub synchronous: Option<String>,
    pub busy_timeout: Option<u64>,
    pub foreign_keys: Option<bool>,
}

impl SqliteConfigInfo {
    pub fn has_config(&self) -> bool {
        self.create_if_missing.is_some()
            || self.read_only.is_some()
            || self.journal_mode.is_some()
            || self.synchronous.is_some()
            || self.busy_timeout.is_some()
            || self.foreign_keys.is_some()
    }
}

/// Collect context fields from the manifest into code generation info.
///
/// This is a shared utility to avoid duplicating the collection logic
/// in each language generator.
pub fn collect_context_fields(context: &Context) -> Vec<ContextFieldInfo> {
    context
        .fields()
        .into_iter()
        .map(|(name, field)| {
            let env_var = field
                .env()
                .map(|s| s.to_string())
                .unwrap_or_else(|| field.default_env().to_string());

            let pool = field
                .pool_config()
                .map(|p| PoolConfigInfo {
                    max_connections: p.max_connections,
                    min_connections: p.min_connections,
                    acquire_timeout: p.acquire_timeout,
                    idle_timeout: p.idle_timeout,
                    max_lifetime: p.max_lifetime,
                })
                .unwrap_or_default();

            let sqlite = field.sqlite_config().map(|s| SqliteConfigInfo {
                path: s.path.clone(),
                create_if_missing: s.create_if_missing,
                read_only: s.read_only,
                journal_mode: s.journal_mode.as_ref().map(|m| m.as_str().to_string()),
                synchronous: s.synchronous.as_ref().map(|m| m.as_str().to_string()),
                busy_timeout: s.busy_timeout,
                foreign_keys: s.foreign_keys,
            });

            // Convert schema ContextField to core ContextFieldType
            let field_type = match &field {
                ContextField::Postgres(_) => ContextFieldType::Database(DatabaseType::Postgres),
                ContextField::Mysql(_) => ContextFieldType::Database(DatabaseType::Mysql),
                ContextField::Sqlite(_) => ContextFieldType::Database(DatabaseType::Sqlite),
                ContextField::Http(_) => ContextFieldType::Http,
            };

            ContextFieldInfo {
                name: name.to_string(),
                field_type,
                env_var,
                is_async: field.is_async(),
                pool,
                sqlite,
            }
        })
        .collect()
}
