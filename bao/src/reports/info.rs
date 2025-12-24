//! Info command report data structures.

use std::path::PathBuf;

use super::output::{Output, Report};

/// Report data from project info.
#[derive(Debug)]
pub struct InfoReport {
    /// CLI name.
    pub name: String,
    /// CLI description.
    pub description: Option<String>,
    /// CLI version.
    pub version: String,
    /// CLI author.
    pub author: Option<String>,
    /// Config file path.
    pub config_path: PathBuf,
    /// Command statistics.
    pub stats: Stats,
    /// Context information.
    pub context: Option<ContextInfo>,
    /// Command tree display.
    pub command_tree: Option<String>,
}

/// Command statistics.
#[derive(Debug, Default)]
pub struct Stats {
    /// Top-level commands.
    pub commands: usize,
    /// Nested subcommands.
    pub subcommands: usize,
    /// Total arguments.
    pub args: usize,
    /// Total flags.
    pub flags: usize,
}

/// Context field information.
#[derive(Debug)]
pub struct ContextInfo {
    /// Database configuration.
    pub database: Option<DatabaseInfo>,
    /// HTTP client configuration.
    pub http: Option<HttpInfo>,
}

/// Database context info.
#[derive(Debug)]
pub struct DatabaseInfo {
    /// Database type (PostgreSQL, MySQL, SQLite).
    pub db_type: String,
    /// Environment variable.
    pub env_var: Option<String>,
    /// Max connections.
    pub max_connections: Option<u32>,
    /// Extra info lines.
    pub extra: Vec<String>,
}

/// HTTP client context info.
#[derive(Debug)]
pub struct HttpInfo {
    /// Timeout in seconds.
    pub timeout: Option<u64>,
    /// User agent.
    pub user_agent: Option<String>,
}

impl Report for InfoReport {
    fn render(&self, out: &mut dyn Output) {
        out.newline();

        // Header
        out.preformatted(&format!("  {}", self.name));
        out.preformatted(&format!("  {}", "─".repeat(self.name.len())));
        if let Some(desc) = &self.description {
            out.preformatted(&format!("  {}", desc));
        }
        out.newline();

        // Metadata
        out.preformatted(&format!("  Version     {}", self.version));
        if let Some(author) = &self.author {
            out.preformatted(&format!("  Author      {}", author));
        }
        out.preformatted(&format!("  Config      {}", self.config_path.display()));
        out.newline();

        // Statistics
        out.preformatted("  Statistics");
        out.preformatted("  ──────────");
        let nested = if self.stats.subcommands > 0 {
            format!(" ({} nested)", self.stats.subcommands)
        } else {
            String::new()
        };
        out.preformatted(&format!("  Commands    {}{}", self.stats.commands, nested));
        out.preformatted(&format!("  Arguments   {}", self.stats.args));
        out.preformatted(&format!("  Flags       {}", self.stats.flags));
        out.newline();

        // Context
        if let Some(context) = &self.context {
            out.preformatted("  Context");
            out.preformatted("  ───────");

            if let Some(db) = &context.database {
                let env = db
                    .env_var
                    .as_ref()
                    .map(|e| format!(" ({})", e))
                    .unwrap_or_default();
                out.preformatted(&format!("  database    {}{}", db.db_type, env));
                if let Some(max) = db.max_connections {
                    out.preformatted(&format!("              └─ max connections: {}", max));
                }
                for extra in &db.extra {
                    out.preformatted(&format!("              └─ {}", extra));
                }
            }

            if let Some(http) = &context.http {
                let timeout = http
                    .timeout
                    .map(|t| format!(" ({}s timeout)", t))
                    .unwrap_or_default();
                out.preformatted(&format!("  http        reqwest::Client{}", timeout));
                if let Some(ua) = &http.user_agent {
                    out.preformatted(&format!("              └─ user-agent: {}", ua));
                }
            }
            out.newline();
        }

        // Command tree
        if let Some(tree) = &self.command_tree {
            out.preformatted("  Commands");
            out.preformatted("  ────────");
            out.preformatted(tree);
        }
    }
}
