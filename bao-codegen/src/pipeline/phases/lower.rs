//! Lower phase - transforms manifest to Application IR.
//!
//! This module transforms the parsed manifest into the unified Application IR
//! that generators consume.

use std::{collections::HashMap, time::Duration};

use baobao_ir::{
    AppIR, AppMeta, CommandOp, DatabaseResource, DatabaseType, DefaultValue, HttpClientResource,
    Input, InputKind, InputType, Operation, PoolConfig, Resource, SqliteOptions,
};
use baobao_manifest::{ArgType, Command, ContextField, Flag, Manifest};
use eyre::Result;

use crate::pipeline::{CompilationContext, Phase};

/// Phase that transforms the manifest into Application IR.
///
/// This phase converts the parsed manifest into the unified IR that generators consume.
pub struct LowerPhase;

impl Phase for LowerPhase {
    fn name(&self) -> &'static str {
        "lower"
    }

    fn description(&self) -> &'static str {
        "Transform Manifest to Application IR"
    }

    fn run(&self, ctx: &mut CompilationContext) -> Result<()> {
        ctx.ir = Some(lower_manifest(&ctx.manifest));
        Ok(())
    }
}

/// Lower a manifest into an Application IR.
fn lower_manifest(manifest: &Manifest) -> AppIR {
    AppIR {
        meta: lower_meta(manifest),
        resources: lower_resources(manifest),
        operations: lower_commands(&manifest.commands),
    }
}

/// Lower CLI metadata from manifest.
fn lower_meta(manifest: &Manifest) -> AppMeta {
    AppMeta {
        name: manifest.cli.name.clone(),
        version: manifest.cli.version.to_string(),
        description: manifest.cli.description.clone(),
        author: manifest.cli.author.clone(),
    }
}

/// Lower context resources from manifest.
fn lower_resources(manifest: &Manifest) -> Vec<Resource> {
    let mut resources = Vec::new();

    if let Some(db) = &manifest.context.database
        && let Some(resource) = lower_database_resource("db", db)
    {
        resources.push(Resource::Database(resource));
    }

    if manifest.context.http.is_some() {
        resources.push(Resource::HttpClient(HttpClientResource {
            name: "http".into(),
        }));
    }

    resources
}

/// Lower a database context field to a DatabaseResource.
fn lower_database_resource(name: &str, field: &ContextField) -> Option<DatabaseResource> {
    let (db_type, env_var, pool_config, sqlite_opts) = match field {
        ContextField::Postgres(config) => (
            DatabaseType::Postgres,
            default_env_var(config.env(), "DATABASE_URL"),
            lower_pool_config(config.pool()),
            None,
        ),
        ContextField::Mysql(config) => (
            DatabaseType::Mysql,
            default_env_var(config.env(), "DATABASE_URL"),
            lower_pool_config(config.pool()),
            None,
        ),
        ContextField::Sqlite(config) => (
            DatabaseType::Sqlite,
            default_env_var(config.env.as_deref(), "DATABASE_URL"),
            lower_pool_config(&config.pool),
            Some(lower_sqlite_options(config)),
        ),
        ContextField::Http(_) => return None,
    };

    Some(DatabaseResource {
        name: name.into(),
        db_type,
        env_var,
        pool: pool_config,
        sqlite: sqlite_opts,
    })
}

/// Get the environment variable or use default.
fn default_env_var(env: Option<&str>, default: &str) -> String {
    env.unwrap_or(default).into()
}

/// Lower pool configuration from manifest format.
fn lower_pool_config(config: &baobao_manifest::PoolConfig) -> PoolConfig {
    PoolConfig {
        max_connections: config.max_connections,
        min_connections: config.min_connections,
        acquire_timeout: config.acquire_timeout.map(Duration::from_secs),
        idle_timeout: config.idle_timeout.map(Duration::from_secs),
        max_lifetime: config.max_lifetime.map(Duration::from_secs),
    }
}

/// Lower SQLite-specific options.
fn lower_sqlite_options(config: &baobao_manifest::SqliteConfig) -> SqliteOptions {
    SqliteOptions {
        path: config.path.clone(),
        create_if_missing: config.create_if_missing,
        read_only: config.read_only,
        journal_mode: config.journal_mode.as_ref().map(|m| match m {
            baobao_manifest::JournalMode::Wal => baobao_ir::JournalMode::Wal,
            baobao_manifest::JournalMode::Delete => baobao_ir::JournalMode::Delete,
            baobao_manifest::JournalMode::Truncate => baobao_ir::JournalMode::Truncate,
            baobao_manifest::JournalMode::Persist => baobao_ir::JournalMode::Persist,
            baobao_manifest::JournalMode::Memory => baobao_ir::JournalMode::Memory,
            baobao_manifest::JournalMode::Off => baobao_ir::JournalMode::Off,
        }),
        synchronous: config.synchronous.as_ref().map(|s| match s {
            baobao_manifest::SynchronousMode::Off => baobao_ir::SynchronousMode::Off,
            baobao_manifest::SynchronousMode::Normal => baobao_ir::SynchronousMode::Normal,
            baobao_manifest::SynchronousMode::Full => baobao_ir::SynchronousMode::Full,
        }),
        busy_timeout: config.busy_timeout.map(Duration::from_millis),
        foreign_keys: config.foreign_keys,
    }
}

/// Lower commands to operations.
fn lower_commands(commands: &HashMap<String, Command>) -> Vec<Operation> {
    // Sort commands for deterministic output
    let mut names: Vec<_> = commands.keys().collect();
    names.sort();

    names
        .into_iter()
        .map(|name| {
            let cmd = &commands[name];
            Operation::Command(lower_command(name, cmd, vec![name.clone()]))
        })
        .collect()
}

/// Lower a single command.
fn lower_command(name: &str, cmd: &Command, path: Vec<String>) -> CommandOp {
    let mut inputs = Vec::new();

    // Lower positional arguments (sorted for deterministic output)
    let mut arg_names: Vec<_> = cmd.args.keys().collect();
    arg_names.sort();
    for arg_name in arg_names {
        let arg = &cmd.args[arg_name];
        inputs.push(Input {
            name: arg_name.clone(),
            ty: lower_arg_type(&arg.arg_type),
            kind: InputKind::Positional,
            required: arg.required,
            default: arg.default.as_ref().and_then(lower_default_value),
            description: arg.description.clone(),
            choices: arg.choices.clone(),
        });
    }

    // Lower flags (sorted for deterministic output)
    let mut flag_names: Vec<_> = cmd.flags.keys().collect();
    flag_names.sort();
    for flag_name in flag_names {
        let flag = &cmd.flags[flag_name];
        inputs.push(lower_flag(flag_name, flag));
    }

    // Lower subcommands
    let mut child_names: Vec<_> = cmd.commands.keys().collect();
    child_names.sort();
    let children: Vec<_> = child_names
        .into_iter()
        .map(|child_name| {
            let child_cmd = &cmd.commands[child_name];
            let mut child_path = path.clone();
            child_path.push(child_name.clone());
            lower_command(child_name, child_cmd, child_path)
        })
        .collect();

    CommandOp {
        name: name.into(),
        path,
        description: cmd.description.clone(),
        inputs,
        children,
    }
}

/// Lower a flag to an Input.
fn lower_flag(name: &str, flag: &Flag) -> Input {
    Input {
        name: name.into(),
        ty: lower_arg_type(&flag.flag_type),
        kind: InputKind::Flag {
            short: flag.short.as_ref().map(|s| *s.get_ref()),
        },
        required: false,
        default: flag.default.as_ref().and_then(lower_default_value),
        description: flag.description.clone(),
        choices: flag.choices.clone(),
    }
}

/// Lower argument type.
fn lower_arg_type(ty: &ArgType) -> InputType {
    match ty {
        ArgType::String => InputType::String,
        ArgType::Int => InputType::Int,
        ArgType::Float => InputType::Float,
        ArgType::Bool => InputType::Bool,
        ArgType::Path => InputType::Path,
    }
}

/// Lower a TOML value to a DefaultValue.
fn lower_default_value(value: &toml::Value) -> Option<DefaultValue> {
    match value {
        toml::Value::String(s) => Some(DefaultValue::String(s.clone())),
        toml::Value::Integer(i) => Some(DefaultValue::Int(*i)),
        toml::Value::Float(f) => Some(DefaultValue::Float(*f)),
        toml::Value::Boolean(b) => Some(DefaultValue::Bool(*b)),
        _ => None, // Arrays, tables, etc. are not supported as defaults
    }
}

#[cfg(test)]
mod tests {
    use baobao_manifest::Manifest;

    use super::*;

    fn parse_manifest(content: &str) -> Manifest {
        toml::from_str(content).expect("Failed to parse test manifest")
    }

    fn make_test_manifest() -> Manifest {
        parse_manifest(
            r#"
            [cli]
            name = "test"
            version = "1.0.0"
            description = "Test CLI"
            language = "rust"
        "#,
        )
    }

    #[test]
    fn test_lower_phase() {
        let manifest = make_test_manifest();
        let mut ctx = CompilationContext::new(manifest);

        assert!(ctx.ir.is_none());

        LowerPhase.run(&mut ctx).expect("lower should succeed");

        assert!(ctx.ir.is_some());

        let ir = ctx.ir.as_ref().unwrap();
        assert_eq!(ir.meta.name, "test");
        assert_eq!(ir.meta.version, "1.0.0");
    }

    #[test]
    fn test_lower_arg_type() {
        assert_eq!(lower_arg_type(&ArgType::String), InputType::String);
        assert_eq!(lower_arg_type(&ArgType::Int), InputType::Int);
        assert_eq!(lower_arg_type(&ArgType::Float), InputType::Float);
        assert_eq!(lower_arg_type(&ArgType::Bool), InputType::Bool);
        assert_eq!(lower_arg_type(&ArgType::Path), InputType::Path);
    }

    #[test]
    fn test_lower_default_value() {
        assert_eq!(
            lower_default_value(&toml::Value::String("hello".into())),
            Some(DefaultValue::String("hello".into()))
        );
        assert_eq!(
            lower_default_value(&toml::Value::Integer(42)),
            Some(DefaultValue::Int(42))
        );
        assert_eq!(
            lower_default_value(&toml::Value::Boolean(true)),
            Some(DefaultValue::Bool(true))
        );
    }

    #[test]
    fn test_lower_pool_config() {
        let manifest_config = baobao_manifest::PoolConfig {
            max_connections: Some(10),
            min_connections: Some(2),
            acquire_timeout: Some(30),
            idle_timeout: Some(600),
            max_lifetime: Some(1800),
        };

        let ir_config = lower_pool_config(&manifest_config);

        assert_eq!(ir_config.max_connections, Some(10));
        assert_eq!(ir_config.min_connections, Some(2));
        assert_eq!(ir_config.acquire_timeout, Some(Duration::from_secs(30)));
        assert_eq!(ir_config.idle_timeout, Some(Duration::from_secs(600)));
        assert_eq!(ir_config.max_lifetime, Some(Duration::from_secs(1800)));
    }
}
