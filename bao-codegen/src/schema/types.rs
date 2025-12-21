//! Types for code generation.

use std::time::Duration;

use baobao_ir::{
    AppIR, CommandOp, ContextFieldType, DatabaseType, JournalMode, Operation, PoolConfig, Resource,
    SqliteOptions, SynchronousMode,
};
use baobao_manifest::{Context, ContextField};

/// Info about a command for code generation.
#[derive(Debug, Clone)]
pub struct CommandInfo {
    pub name: String,
    pub description: String,
    pub has_subcommands: bool,
}

/// Info about a context field for code generation.
#[derive(Debug, Clone)]
pub struct ContextFieldInfo {
    pub name: String,
    /// Language-agnostic field type.
    pub field_type: ContextFieldType,
    pub env_var: String,
    pub is_async: bool,
    pub pool: PoolConfig,
    pub sqlite: Option<SqliteOptions>,
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
                .map(|p| PoolConfig {
                    max_connections: p.max_connections,
                    min_connections: p.min_connections,
                    acquire_timeout: p.acquire_timeout.map(Duration::from_secs),
                    idle_timeout: p.idle_timeout.map(Duration::from_secs),
                    max_lifetime: p.max_lifetime.map(Duration::from_secs),
                })
                .unwrap_or_default();

            let sqlite = field.sqlite_config().map(|s| SqliteOptions {
                path: s.path.clone(),
                create_if_missing: s.create_if_missing,
                read_only: s.read_only,
                journal_mode: s.journal_mode.as_ref().map(convert_journal_mode),
                synchronous: s.synchronous.as_ref().map(convert_synchronous_mode),
                busy_timeout: s.busy_timeout.map(Duration::from_millis),
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

/// Convert manifest JournalMode to IR JournalMode.
fn convert_journal_mode(mode: &baobao_manifest::JournalMode) -> JournalMode {
    match mode {
        baobao_manifest::JournalMode::Wal => JournalMode::Wal,
        baobao_manifest::JournalMode::Delete => JournalMode::Delete,
        baobao_manifest::JournalMode::Truncate => JournalMode::Truncate,
        baobao_manifest::JournalMode::Persist => JournalMode::Persist,
        baobao_manifest::JournalMode::Memory => JournalMode::Memory,
        baobao_manifest::JournalMode::Off => JournalMode::Off,
    }
}

/// Convert manifest SynchronousMode to IR SynchronousMode.
fn convert_synchronous_mode(mode: &baobao_manifest::SynchronousMode) -> SynchronousMode {
    match mode {
        baobao_manifest::SynchronousMode::Off => SynchronousMode::Off,
        baobao_manifest::SynchronousMode::Normal => SynchronousMode::Normal,
        baobao_manifest::SynchronousMode::Full => SynchronousMode::Full,
    }
}

// ============================================================================
// IR-based helper functions
// ============================================================================

/// Collect context fields from AppIR resources.
pub fn collect_context_fields_from_ir(ir: &AppIR) -> Vec<ContextFieldInfo> {
    ir.resources
        .iter()
        .map(|resource| match resource {
            Resource::Database(db) => ContextFieldInfo {
                name: db.name.clone(),
                field_type: ContextFieldType::Database(db.db_type),
                env_var: db.env_var.clone(),
                is_async: true, // Database operations are always async
                pool: db.pool.clone(),
                sqlite: db.sqlite.clone(),
            },
            Resource::HttpClient(http) => ContextFieldInfo {
                name: http.name.clone(),
                field_type: ContextFieldType::Http,
                env_var: String::new(), // HTTP client doesn't need env var
                is_async: false,        // HTTP client creation is sync
                pool: PoolConfig::default(),
                sqlite: None,
            },
        })
        .collect()
}

/// Collect command info from AppIR operations.
#[allow(clippy::unnecessary_filter_map)] // Operation will have more variants
pub fn collect_commands_from_ir(ir: &AppIR) -> Vec<CommandInfo> {
    ir.operations
        .iter()
        .filter_map(|op| match op {
            Operation::Command(cmd) => Some(CommandInfo {
                name: cmd.name.clone(),
                description: cmd.description.clone(),
                has_subcommands: cmd.has_subcommands(),
            }),
        })
        .collect()
}

/// Check if the AppIR has any async resources.
pub fn ir_has_async(ir: &AppIR) -> bool {
    ir.resources
        .iter()
        .any(|r| matches!(r, Resource::Database(_)))
}

/// Collect all command paths from AppIR (for orphan detection).
pub fn collect_command_paths_from_ir(ir: &AppIR) -> Vec<String> {
    fn collect_paths(cmd: &CommandOp, paths: &mut Vec<String>) {
        paths.push(cmd.handler_path());
        for child in &cmd.children {
            collect_paths(child, paths);
        }
    }

    let mut paths = Vec::new();
    for op in &ir.operations {
        let Operation::Command(cmd) = op;
        collect_paths(cmd, &mut paths);
    }
    paths
}
