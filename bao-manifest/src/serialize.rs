//! Serialization support for formatting bao.toml files.
//!
//! This module provides serializable versions of the manifest types that
//! output canonical, sorted TOML.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::{
    ArgType, CliConfig, Command, Context, ContextField, HttpConfig, JournalMode, Manifest,
    SynchronousMode,
};

/// Serializable manifest for canonical TOML output.
///
/// Fields are ordered: cli, context, commands
#[derive(Debug, Serialize)]
pub struct SerializableManifest {
    pub cli: SerializableCliConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<SerializableContext>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub commands: BTreeMap<String, SerializableCommand>,
}

impl From<&Manifest> for SerializableManifest {
    fn from(m: &Manifest) -> Self {
        Self {
            cli: SerializableCliConfig::from(&m.cli),
            context: if m.context.is_empty() {
                None
            } else {
                Some(SerializableContext::from(&m.context))
            },
            commands: m
                .commands
                .iter()
                .map(|(k, v)| (k.clone(), SerializableCommand::from(v)))
                .collect(),
        }
    }
}

/// Serializable CLI configuration.
///
/// Fields ordered: name, version, author, description
#[derive(Debug, Serialize)]
pub struct SerializableCliConfig {
    pub name: String,
    #[serde(skip_serializing_if = "is_default_version")]
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn is_default_version(v: &str) -> bool {
    v == "0.1.0"
}

impl From<&CliConfig> for SerializableCliConfig {
    fn from(c: &CliConfig) -> Self {
        Self {
            name: c.name.clone(),
            version: c.version.clone(),
            author: c.author.clone(),
            description: c.description.clone(),
        }
    }
}

/// Serializable context configuration.
///
/// Fields ordered: database, http
#[derive(Debug, Serialize)]
pub struct SerializableContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<SerializableDatabaseConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http: Option<SerializableHttpConfig>,
}

impl From<&Context> for SerializableContext {
    fn from(c: &Context) -> Self {
        Self {
            database: c.database.as_ref().map(SerializableDatabaseConfig::from),
            http: c
                .http
                .as_ref()
                .and_then(|f| f.http_config())
                .map(SerializableHttpConfig::from),
        }
    }
}

/// Serializable database configuration with type tag.
#[derive(Debug, Serialize)]
pub struct SerializableDatabaseConfig {
    #[serde(rename = "type")]
    pub db_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    // Pool config (flattened in original, explicit here for sorting)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acquire_timeout: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_timeout: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_connections: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_lifetime: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_connections: Option<u32>,
    // SQLite-specific options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub busy_timeout: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_if_missing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub foreign_keys: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journal_mode: Option<JournalMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synchronous: Option<SynchronousMode>,
}

impl From<&ContextField> for SerializableDatabaseConfig {
    fn from(field: &ContextField) -> Self {
        match field {
            ContextField::Postgres(c) => Self {
                db_type: "postgres".to_string(),
                env: c.0.env.clone(),
                path: None,
                acquire_timeout: c.0.pool.acquire_timeout,
                idle_timeout: c.0.pool.idle_timeout,
                max_connections: c.0.pool.max_connections,
                max_lifetime: c.0.pool.max_lifetime,
                min_connections: c.0.pool.min_connections,
                busy_timeout: None,
                create_if_missing: None,
                foreign_keys: None,
                journal_mode: None,
                read_only: None,
                synchronous: None,
            },
            ContextField::Mysql(c) => Self {
                db_type: "mysql".to_string(),
                env: c.0.env.clone(),
                path: None,
                acquire_timeout: c.0.pool.acquire_timeout,
                idle_timeout: c.0.pool.idle_timeout,
                max_connections: c.0.pool.max_connections,
                max_lifetime: c.0.pool.max_lifetime,
                min_connections: c.0.pool.min_connections,
                busy_timeout: None,
                create_if_missing: None,
                foreign_keys: None,
                journal_mode: None,
                read_only: None,
                synchronous: None,
            },
            ContextField::Sqlite(c) => Self {
                db_type: "sqlite".to_string(),
                env: c.env.clone(),
                path: c.path.clone(),
                acquire_timeout: c.pool.acquire_timeout,
                idle_timeout: c.pool.idle_timeout,
                max_connections: c.pool.max_connections,
                max_lifetime: c.pool.max_lifetime,
                min_connections: c.pool.min_connections,
                busy_timeout: c.busy_timeout,
                create_if_missing: c.create_if_missing,
                foreign_keys: c.foreign_keys,
                journal_mode: c.journal_mode.clone(),
                read_only: c.read_only,
                synchronous: c.synchronous.clone(),
            },
            ContextField::Http(_) => panic!("HTTP is not a database config"),
        }
    }
}

/// Serializable HTTP configuration.
#[derive(Debug, Serialize)]
pub struct SerializableHttpConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
}

impl From<&HttpConfig> for SerializableHttpConfig {
    fn from(c: &HttpConfig) -> Self {
        Self {
            timeout: c.timeout,
            user_agent: c.user_agent.clone(),
        }
    }
}

/// Serializable command.
///
/// Fields ordered: description, args, commands, flags
#[derive(Debug, Serialize)]
pub struct SerializableCommand {
    pub description: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, SerializableArg>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub commands: BTreeMap<String, SerializableCommand>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub flags: BTreeMap<String, SerializableFlag>,
}

impl From<&Command> for SerializableCommand {
    fn from(c: &Command) -> Self {
        Self {
            description: c.description.clone(),
            args: c
                .args
                .iter()
                .map(|(k, v)| (k.clone(), SerializableArg::from(v)))
                .collect(),
            commands: c
                .commands
                .iter()
                .map(|(k, v)| (k.clone(), SerializableCommand::from(v)))
                .collect(),
            flags: c
                .flags
                .iter()
                .map(|(k, v)| (k.clone(), SerializableFlag::from(v)))
                .collect(),
        }
    }
}

/// Serializable argument.
///
/// Fields ordered: type, default, description, required
#[derive(Debug, Serialize)]
pub struct SerializableArg {
    #[serde(rename = "type")]
    pub arg_type: ArgType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<toml::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "is_true")]
    pub required: bool,
}

fn is_true(v: &bool) -> bool {
    *v
}

impl From<&crate::Arg> for SerializableArg {
    fn from(a: &crate::Arg) -> Self {
        Self {
            arg_type: a.arg_type.clone(),
            default: a.default.clone(),
            description: a.description.clone(),
            required: a.required,
        }
    }
}

/// Serializable flag.
///
/// Fields ordered: type, default, description, short
#[derive(Debug, Serialize)]
pub struct SerializableFlag {
    #[serde(rename = "type", skip_serializing_if = "is_default_flag_type")]
    pub flag_type: ArgType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<toml::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short: Option<char>,
}

fn is_default_flag_type(t: &ArgType) -> bool {
    *t == ArgType::Bool
}

impl From<&crate::Flag> for SerializableFlag {
    fn from(f: &crate::Flag) -> Self {
        Self {
            flag_type: f.flag_type.clone(),
            default: f.default.clone(),
            description: f.description.clone(),
            short: f.short.as_ref().map(|s| *s.get_ref()),
        }
    }
}

/// Convert a manifest to a formatted TOML string.
pub fn to_formatted_string(manifest: &Manifest) -> String {
    let serializable = SerializableManifest::from(manifest);
    toml::to_string_pretty(&serializable).expect("serialization cannot fail for valid manifest")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(content: &str) -> Manifest {
        toml::from_str(content).expect("Failed to parse TOML")
    }

    #[test]
    fn test_round_trip_basic() {
        let input = r#"
[cli]
name = "test"

[commands.hello]
description = "Say hello"
"#;
        let manifest = parse(input);
        let output = to_formatted_string(&manifest);
        let reparsed: Manifest = toml::from_str(&output).expect("Failed to reparse");

        assert_eq!(manifest.cli.name, reparsed.cli.name);
        assert!(reparsed.commands.contains_key("hello"));
    }

    #[test]
    fn test_commands_sorted() {
        let input = r#"
[cli]
name = "test"

[commands.zebra]
description = "Z command"

[commands.alpha]
description = "A command"

[commands.middle]
description = "M command"
"#;
        let manifest = parse(input);
        let output = to_formatted_string(&manifest);

        // Commands should appear in alphabetical order
        let alpha_pos = output.find("[commands.alpha]").unwrap();
        let middle_pos = output.find("[commands.middle]").unwrap();
        let zebra_pos = output.find("[commands.zebra]").unwrap();

        assert!(alpha_pos < middle_pos);
        assert!(middle_pos < zebra_pos);
    }

    #[test]
    fn test_section_order() {
        let input = r#"
[commands.hello]
description = "Say hello"

[context.database]
type = "postgres"

[cli]
name = "test"
"#;
        let manifest = parse(input);
        let output = to_formatted_string(&manifest);

        // cli should come before context which should come before commands
        let cli_pos = output.find("[cli]").unwrap();
        let context_pos = output.find("[context").unwrap();
        let commands_pos = output.find("[commands").unwrap();

        assert!(cli_pos < context_pos);
        assert!(context_pos < commands_pos);
    }

    #[test]
    fn test_empty_context_omitted() {
        let input = r#"
[cli]
name = "test"

[commands.hello]
description = "Say hello"
"#;
        let manifest = parse(input);
        let output = to_formatted_string(&manifest);

        assert!(!output.contains("[context]"));
    }

    #[test]
    fn test_default_values_omitted() {
        let input = r#"
[cli]
name = "test"
version = "0.1.0"

[commands.hello]
description = "Say hello"

[[commands.hello.args]]
name = "target"
type = "string"
required = true

[[commands.hello.flags]]
name = "verbose"
type = "bool"
"#;
        let manifest = parse(input);
        let output = to_formatted_string(&manifest);

        // version = "0.1.0" (default) should be omitted
        assert!(!output.contains("version"));
        // required = true (default) should be omitted
        assert!(!output.contains("required"));
        // type = "bool" for flags (default) should be omitted
        let flags_section = output.find("[commands.hello.flags.verbose]").unwrap();
        let after_flags = &output[flags_section..];
        let next_section = after_flags.find("\n[").unwrap_or(after_flags.len());
        let flag_content = &after_flags[..next_section];
        assert!(!flag_content.contains("type"));
    }

    #[test]
    fn test_idempotent() {
        let input = r#"
[cli]
name = "test"
description = "A test CLI"

[context.database]
type = "sqlite"
env = "DATABASE_URL"

[commands.users]
description = "User management"

[commands.users.commands.create]
description = "Create user"

[[commands.users.commands.create.args]]
name = "name"
type = "string"
"#;
        let manifest = parse(input);
        let first = to_formatted_string(&manifest);
        let reparsed: Manifest = toml::from_str(&first).expect("Failed to reparse");
        let second = to_formatted_string(&reparsed);

        assert_eq!(first, second);
    }

    #[test]
    fn test_nested_commands_sorted() {
        let input = r#"
[cli]
name = "test"

[commands.parent]
description = "Parent command"

[commands.parent.commands.zebra]
description = "Z subcommand"

[commands.parent.commands.alpha]
description = "A subcommand"
"#;
        let manifest = parse(input);
        let output = to_formatted_string(&manifest);

        let alpha_pos = output.find("alpha]").unwrap();
        let zebra_pos = output.find("zebra]").unwrap();

        assert!(alpha_pos < zebra_pos);
    }
}
