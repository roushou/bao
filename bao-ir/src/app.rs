//! Application Intermediate Representation.
//!
//! This module defines the unified IR for CLI applications (and future HTTP servers).
//! The IR serves as a clean abstraction layer between the manifest parsing and
//! language-specific code generation.
//!
//! # Architecture
//!
//! ```text
//! bao.toml → Manifest (parsing) → AppIR (lowering) → Generator (codegen)
//! ```

use serde::Serialize;

use crate::{ContextFieldInfo, ContextFieldType, DatabaseType, PoolConfig, SqliteOptions};

/// Application IR - unified representation for code generation.
#[derive(Debug, Clone, Serialize)]
pub struct AppIR {
    /// Application metadata.
    pub meta: AppMeta,
    /// Shared resources (database, HTTP client, etc.).
    pub resources: Vec<Resource>,
    /// Operations (commands for CLI, routes for HTTP).
    pub operations: Vec<Operation>,
}

impl AppIR {
    /// Returns true if any resource requires async initialization.
    pub fn has_async(&self) -> bool {
        self.resources
            .iter()
            .any(|r| matches!(r, Resource::Database(_)))
    }

    /// Returns true if a database resource is configured.
    pub fn has_database(&self) -> bool {
        self.resources
            .iter()
            .any(|r| matches!(r, Resource::Database(_)))
    }

    /// Returns true if an HTTP client resource is configured.
    pub fn has_http(&self) -> bool {
        self.resources
            .iter()
            .any(|r| matches!(r, Resource::HttpClient(_)))
    }

    /// Iterate over all commands.
    pub fn commands(&self) -> impl Iterator<Item = &CommandOp> {
        self.operations.iter().map(|op| {
            let Operation::Command(cmd) = op;
            cmd
        })
    }

    /// Collect all handler paths from commands (for orphan detection).
    pub fn handler_paths(&self) -> Vec<String> {
        fn collect(cmd: &CommandOp, paths: &mut Vec<String>) {
            paths.push(cmd.handler_path());
            for child in &cmd.children {
                collect(child, paths);
            }
        }

        let mut paths = Vec::new();
        for cmd in self.commands() {
            collect(cmd, &mut paths);
        }
        paths
    }

    /// Count total number of leaf commands (commands without subcommands).
    pub fn command_count(&self) -> usize {
        fn count(cmd: &CommandOp) -> usize {
            if cmd.children.is_empty() {
                1
            } else {
                cmd.children.iter().map(count).sum()
            }
        }

        self.commands().map(count).sum()
    }

    /// Collect context fields from resources.
    pub fn context_fields(&self) -> Vec<ContextFieldInfo> {
        self.resources
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
}

/// Application metadata.
#[derive(Debug, Clone, Serialize)]
pub struct AppMeta {
    /// Application name.
    pub name: String,
    /// Version string.
    pub version: String,
    /// Description for help text.
    pub description: Option<String>,
    /// Author information.
    pub author: Option<String>,
}

/// A shared resource in the application context.
#[derive(Debug, Clone, Serialize)]
pub enum Resource {
    /// Database connection pool.
    Database(DatabaseResource),
    /// HTTP client.
    HttpClient(HttpClientResource),
}

/// Database resource configuration.
#[derive(Debug, Clone, Serialize)]
pub struct DatabaseResource {
    /// Field name in the context struct.
    pub name: String,
    /// Database type (Postgres, MySQL, SQLite).
    pub db_type: DatabaseType,
    /// Environment variable for the connection string.
    pub env_var: String,
    /// Pool configuration.
    pub pool: PoolConfig,
    /// SQLite-specific options.
    pub sqlite: Option<SqliteOptions>,
}

/// HTTP client resource configuration.
#[derive(Debug, Clone, Serialize)]
pub struct HttpClientResource {
    /// Field name in the context struct.
    pub name: String,
}

/// An operation in the application.
#[derive(Debug, Clone, Serialize)]
pub enum Operation {
    /// CLI command.
    Command(CommandOp),
    // Future: Route(RouteOp),
}

/// A CLI command operation.
#[derive(Debug, Clone, Serialize)]
pub struct CommandOp {
    /// Command name.
    pub name: String,
    /// Full path from root (e.g., ["users", "create"]).
    pub path: Vec<String>,
    /// Command description.
    pub description: String,
    /// Input parameters (args and flags).
    pub inputs: Vec<Input>,
    /// Child commands (subcommands).
    pub children: Vec<CommandOp>,
}

impl CommandOp {
    /// Returns true if this command has subcommands.
    pub fn has_subcommands(&self) -> bool {
        !self.children.is_empty()
    }

    /// Returns the handler path (e.g., "users/create" for nested commands).
    pub fn handler_path(&self) -> String {
        self.path.join("/")
    }
}

/// An input parameter for a command.
#[derive(Debug, Clone, Serialize)]
pub struct Input {
    /// Parameter name.
    pub name: String,
    /// Parameter type.
    pub ty: InputType,
    /// Parameter kind (positional or flag).
    pub kind: InputKind,
    /// Whether the parameter is required.
    pub required: bool,
    /// Default value.
    pub default: Option<DefaultValue>,
    /// Description for help text.
    pub description: Option<String>,
    /// Allowed choices (creates enum in generated code).
    pub choices: Option<Vec<String>>,
}

/// Input parameter type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum InputType {
    String,
    Int,
    Float,
    Bool,
    Path,
}

/// Input parameter kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum InputKind {
    /// Positional argument.
    Positional,
    /// Named flag with optional short form.
    Flag {
        /// Short flag character (e.g., 'v' for -v).
        short: Option<char>,
    },
}

/// A default value for an input.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum DefaultValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
}

impl DefaultValue {
    /// Convert to a string representation suitable for code generation.
    pub fn to_code_string(&self) -> String {
        match self {
            Self::String(s) => s.clone(),
            Self::Int(i) => i.to_string(),
            Self::Float(f) => f.to_string(),
            Self::Bool(b) => b.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_has_subcommands() {
        let cmd = CommandOp {
            name: "test".into(),
            path: vec!["test".into()],
            description: "A test command".into(),
            inputs: vec![],
            children: vec![],
        };
        assert!(!cmd.has_subcommands());

        let parent = CommandOp {
            name: "parent".into(),
            path: vec!["parent".into()],
            description: "A parent command".into(),
            inputs: vec![],
            children: vec![cmd],
        };
        assert!(parent.has_subcommands());
    }

    #[test]
    fn test_command_handler_path() {
        let cmd = CommandOp {
            name: "create".into(),
            path: vec!["users".into(), "create".into()],
            description: "Create a user".into(),
            inputs: vec![],
            children: vec![],
        };
        assert_eq!(cmd.handler_path(), "users/create");
    }

    #[test]
    fn test_default_value_to_code_string() {
        assert_eq!(
            DefaultValue::String("hello".into()).to_code_string(),
            "hello"
        );
        assert_eq!(DefaultValue::Int(42).to_code_string(), "42");
        assert_eq!(DefaultValue::Bool(true).to_code_string(), "true");
    }
}
