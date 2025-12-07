/// Info about a command for CLI generation
#[derive(Clone)]
pub struct CommandInfo {
    pub name: String,
    pub description: String,
    pub has_subcommands: bool,
}

/// Info about a context field for code generation
#[derive(Clone)]
pub struct ContextFieldInfo {
    pub name: String,
    pub rust_type: String,
    pub env_var: String,
    pub is_async: bool,
    pub pool: PoolConfigInfo,
    pub sqlite: Option<SqliteConfigInfo>,
}

/// Pool configuration for code generation
#[derive(Clone, Default)]
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
#[derive(Clone, Default)]
pub struct SqliteConfigInfo {
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
