//! CLI framework adapter abstraction.
//!
//! This module defines the [`CliAdapter`] trait for abstracting CLI framework-specific
//! code generation (clap, argh, boune, commander, etc.).

use baobao_core::{ArgType, Version};

use crate::builder::CodeFragment;

/// Dependency specification for an adapter.
#[derive(Debug, Clone)]
pub struct Dependency {
    /// Package/crate name
    pub name: String,
    /// Version specification (e.g., "1.0", "^0.5.0", `{ version = "4", features = ["derive"] }`)
    pub version: String,
    /// Whether this is a dev dependency
    pub dev: bool,
}

impl Dependency {
    /// Create a new runtime dependency.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            dev: false,
        }
    }

    /// Create a new dev dependency.
    pub fn dev(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            dev: true,
        }
    }
}

/// Import specification for generated code.
#[derive(Debug, Clone)]
pub struct ImportSpec {
    /// Module/package path
    pub module: String,
    /// Symbols to import (empty = import module itself)
    pub symbols: Vec<String>,
    /// Whether this is a type-only import (TypeScript)
    pub type_only: bool,
}

impl ImportSpec {
    /// Create a new import specification.
    pub fn new(module: impl Into<String>) -> Self {
        Self {
            module: module.into(),
            symbols: Vec::new(),
            type_only: false,
        }
    }

    /// Add a symbol to import.
    pub fn symbol(mut self, symbol: impl Into<String>) -> Self {
        self.symbols.push(symbol.into());
        self
    }

    /// Add multiple symbols to import.
    pub fn symbols(mut self, symbols: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.symbols.extend(symbols.into_iter().map(Into::into));
        self
    }

    /// Mark as type-only import (for TypeScript).
    pub fn type_only(mut self) -> Self {
        self.type_only = true;
        self
    }
}

/// Framework-agnostic CLI application info.
#[derive(Debug, Clone)]
pub struct CliInfo {
    /// Application name
    pub name: String,
    /// Application version
    pub version: Version,
    /// Application description
    pub description: Option<String>,
    /// Top-level commands
    pub commands: Vec<CommandMeta>,
    /// Whether any command uses async
    pub is_async: bool,
}

/// Framework-agnostic command metadata.
#[derive(Debug, Clone)]
pub struct CommandMeta {
    /// Command name (e.g., "hello", "db")
    pub name: String,
    /// Pascal case name (e.g., "Hello", "Db")
    pub pascal_name: String,
    /// Snake case name (e.g., "hello", "db")
    pub snake_name: String,
    /// Command description
    pub description: String,
    /// Positional arguments
    pub args: Vec<ArgMeta>,
    /// Optional flags
    pub flags: Vec<FlagMeta>,
    /// Whether this command has subcommands
    pub has_subcommands: bool,
    /// Subcommands (if any)
    pub subcommands: Vec<SubcommandMeta>,
}

/// Framework-agnostic positional argument metadata.
#[derive(Debug, Clone)]
pub struct ArgMeta {
    /// Argument name
    pub name: String,
    /// Snake case name for field
    pub field_name: String,
    /// Argument type
    pub arg_type: ArgType,
    /// Whether this argument is required
    pub required: bool,
    /// Default value (if any)
    pub default: Option<String>,
    /// Argument description
    pub description: Option<String>,
}

/// Framework-agnostic flag metadata.
#[derive(Debug, Clone)]
pub struct FlagMeta {
    /// Flag name (long form, e.g., "verbose")
    pub name: String,
    /// Snake case name for field
    pub field_name: String,
    /// Short flag character (e.g., 'v')
    pub short: Option<char>,
    /// Flag type
    pub flag_type: ArgType,
    /// Default value (if any)
    pub default: Option<String>,
    /// Flag description
    pub description: Option<String>,
}

/// Framework-agnostic subcommand metadata.
#[derive(Debug, Clone)]
pub struct SubcommandMeta {
    /// Subcommand name
    pub name: String,
    /// Pascal case name
    pub pascal_name: String,
    /// Snake case name
    pub snake_name: String,
    /// Subcommand description
    pub description: String,
    /// Whether this subcommand has its own subcommands
    pub has_subcommands: bool,
}

/// Info needed to generate command dispatch logic.
#[derive(Debug, Clone)]
pub struct DispatchInfo {
    /// Parent command name (pascal case)
    pub parent_name: String,
    /// Subcommands to dispatch to
    pub subcommands: Vec<SubcommandMeta>,
    /// Handler module path prefix
    pub handler_path: String,
    /// Whether dispatch is async
    pub is_async: bool,
}

/// Trait for CLI framework adapters.
///
/// Implement this trait to support a specific CLI framework (clap, boune, etc.).
pub trait CliAdapter {
    /// Adapter name for identification.
    fn name(&self) -> &'static str;

    /// Dependencies required by this adapter.
    fn dependencies(&self) -> Vec<Dependency>;

    /// Generate the main CLI entry point structure/definition.
    fn generate_cli(&self, info: &CliInfo) -> Vec<CodeFragment>;

    /// Generate a command definition (args struct, command object, etc.).
    fn generate_command(&self, info: &CommandMeta) -> Vec<CodeFragment>;

    /// Generate subcommand enum/routing for a parent command.
    fn generate_subcommands(&self, info: &CommandMeta) -> Vec<CodeFragment>;

    /// Generate dispatch logic for routing to handlers.
    fn generate_dispatch(&self, info: &DispatchInfo) -> Vec<CodeFragment>;

    /// Imports needed for CLI code.
    fn imports(&self) -> Vec<ImportSpec>;

    /// Imports needed for a specific command.
    fn command_imports(&self, info: &CommandMeta) -> Vec<ImportSpec>;

    /// Map an argument type to the adapter's type string.
    fn map_arg_type(&self, arg_type: ArgType) -> &'static str;

    /// Map an optional argument type to the adapter's type string.
    fn map_optional_type(&self, arg_type: ArgType) -> String;
}
