use std::{collections::HashMap, path::PathBuf};

use baobao_manifest::{BaoToml, Command, ContextField};
use clap::Args;
use eyre::Result;

use super::UnwrapOrExit;

#[derive(Args)]
pub struct InfoCommand {
    /// Path to bao.toml (defaults to ./bao.toml)
    #[arg(short, long, default_value = "bao.toml")]
    pub config: PathBuf,
}

impl InfoCommand {
    pub fn run(&self) -> Result<()> {
        let bao_toml = BaoToml::open(&self.config).unwrap_or_exit();
        let schema = bao_toml.schema();

        // Header
        println!();
        println!("  {}", schema.cli.name);
        println!("  {}", "─".repeat(schema.cli.name.len()));
        if let Some(desc) = &schema.cli.description {
            println!("  {}", desc);
        }
        println!();

        // Metadata
        println!("  Version     {}", schema.cli.version);
        if let Some(author) = &schema.cli.author {
            println!("  Author      {}", author);
        }
        println!(
            "  Config      {}",
            std::fs::canonicalize(&self.config)
                .unwrap_or_else(|_| self.config.clone())
                .display()
        );
        println!();

        // Statistics
        let stats = collect_stats(&schema.commands);
        println!("  Statistics");
        println!("  ──────────");
        println!(
            "  Commands    {}{}",
            stats.commands,
            if stats.subcommands > 0 {
                format!(" ({} nested)", stats.subcommands)
            } else {
                String::new()
            }
        );
        println!("  Arguments   {}", stats.args);
        println!("  Flags       {}", stats.flags);
        println!();

        // Context
        if !schema.context.is_empty() {
            println!("  Context");
            println!("  ───────");
            if let Some(db) = &schema.context.database {
                print_database_info(db);
            }
            if let Some(http) = &schema.context.http {
                print_http_info(http);
            }
            println!();
        }

        // Command tree
        if !schema.commands.is_empty() {
            println!("  Commands");
            println!("  ────────");
            print_command_tree(&schema.commands, "  ");
        }

        Ok(())
    }
}

struct Stats {
    commands: usize,
    subcommands: usize,
    args: usize,
    flags: usize,
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

fn print_database_info(db: &ContextField) {
    let (db_type, env, pool) = match db {
        ContextField::Postgres(c) => ("PostgreSQL", c.env(), Some(c.pool())),
        ContextField::Mysql(c) => ("MySQL", c.env(), Some(c.pool())),
        ContextField::Sqlite(c) => ("SQLite", c.env.as_deref(), Some(&c.pool)),
        _ => return,
    };

    print!("  database    {}", db_type);
    if let Some(env) = env {
        print!(" ({})", env);
    }
    println!();

    if let Some(pool) = pool
        && pool.has_config()
        && let Some(max) = pool.max_connections
    {
        println!("              └─ max connections: {}", max);
    }

    // SQLite-specific info
    if let ContextField::Sqlite(c) = db {
        if let Some(path) = &c.path {
            println!("              └─ path: {}", path);
        }
        if let Some(mode) = &c.journal_mode {
            println!("              └─ journal: {}", mode.as_str().to_lowercase());
        }
    }
}

fn print_http_info(http: &ContextField) {
    if let Some(config) = http.http_config() {
        print!("  http        reqwest::Client");
        if let Some(timeout) = config.timeout {
            print!(" ({}s timeout)", timeout);
        }
        println!();
        if let Some(ua) = &config.user_agent {
            println!("              └─ user-agent: {}", ua);
        }
    }
}

fn print_command_tree(commands: &HashMap<String, Command>, indent: &str) {
    let mut names: Vec<_> = commands.keys().collect();
    names.sort();

    for (i, name) in names.iter().enumerate() {
        let cmd = &commands[*name];
        let is_last = i == names.len() - 1;
        let prefix = if is_last { "└─" } else { "├─" };
        let child_indent = if is_last { "   " } else { "│  " };

        // Command name and description
        print!("{}{} {}", indent, prefix, name);

        // Show args/flags count inline
        let mut meta = Vec::new();
        if !cmd.args.is_empty() {
            meta.push(format!(
                "{} arg{}",
                cmd.args.len(),
                if cmd.args.len() == 1 { "" } else { "s" }
            ));
        }
        if !cmd.flags.is_empty() {
            meta.push(format!(
                "{} flag{}",
                cmd.flags.len(),
                if cmd.flags.len() == 1 { "" } else { "s" }
            ));
        }

        if meta.is_empty() {
            println!();
        } else {
            println!(" ({})", meta.join(", "));
        }

        // Recurse into subcommands
        if cmd.has_subcommands() {
            print_command_tree(&cmd.commands, &format!("{}{}", indent, child_indent));
        }
    }
}
