//! Info operation - project information.

use std::{collections::HashMap, path::Path};

use baobao_codegen::schema::{CommandTree, DisplayStyle};
use baobao_manifest::{Command, ContextField, Manifest};

use crate::reports::{ContextInfo, InfoReport, Stats};

/// Execute the info operation.
///
/// Collects project information from the manifest.
pub fn info(manifest: &Manifest, config_path: &Path) -> InfoReport {
    let stats = collect_stats(&manifest.commands);
    let context = collect_context(manifest);
    let command_tree = if manifest.commands.is_empty() {
        None
    } else {
        let tree = CommandTree::new(manifest);
        Some(
            tree.display_style(DisplayStyle::TreeBox)
                .indent("  ")
                .to_string(),
        )
    };

    InfoReport {
        name: manifest.cli.name.clone(),
        description: manifest.cli.description.clone(),
        version: manifest.cli.version.to_string(),
        author: manifest.cli.author.clone(),
        config_path: std::fs::canonicalize(config_path)
            .unwrap_or_else(|_| config_path.to_path_buf()),
        stats,
        context,
        command_tree,
    }
}

fn collect_stats(commands: &HashMap<String, Command>) -> Stats {
    let mut stats = Stats {
        commands: 0,
        subcommands: 0,
        args: 0,
        flags: 0,
    };
    collect_stats_recursive(commands, &mut stats, 0);
    stats
}

fn collect_stats_recursive(commands: &HashMap<String, Command>, stats: &mut Stats, depth: usize) {
    for cmd in commands.values() {
        if depth == 0 {
            stats.commands += 1;
        } else {
            stats.subcommands += 1;
        }
        stats.args += cmd.args.len();
        stats.flags += cmd.flags.len();
        collect_stats_recursive(&cmd.commands, stats, depth + 1);
    }
}

fn collect_context(manifest: &Manifest) -> Option<ContextInfo> {
    if manifest.context.is_empty() {
        return None;
    }

    let database = manifest.context.database.as_ref().map(|db| {
        let (db_type, env, max_conn) = match db {
            ContextField::Postgres(c) => (
                "PostgreSQL",
                c.env().map(String::from),
                c.pool().max_connections,
            ),
            ContextField::Mysql(c) => {
                ("MySQL", c.env().map(String::from), c.pool().max_connections)
            }
            ContextField::Sqlite(c) => ("SQLite", c.env.clone(), c.pool.max_connections),
            _ => {
                return crate::reports::DatabaseInfo {
                    db_type: "Unknown".to_string(),
                    env_var: None,
                    max_connections: None,
                    extra: Vec::new(),
                };
            }
        };

        let mut extra = Vec::new();
        if let ContextField::Sqlite(c) = db {
            if let Some(path) = &c.path {
                extra.push(format!("path: {}", path));
            }
            if let Some(mode) = &c.journal_mode {
                extra.push(format!("journal: {}", mode.as_str().to_lowercase()));
            }
        }

        crate::reports::DatabaseInfo {
            db_type: db_type.to_string(),
            env_var: env,
            max_connections: max_conn,
            extra,
        }
    });

    let http = manifest.context.http.as_ref().and_then(|h| {
        h.http_config().map(|config| crate::reports::HttpInfo {
            timeout: config.timeout,
            user_agent: config.user_agent.clone(),
        })
    });

    Some(ContextInfo { database, http })
}
