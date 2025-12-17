use std::path::{Path, PathBuf};

use baobao_manifest::{BaoToml, rename_command_section};
use clap::{Args, Subcommand};
use eyre::{Context, Result};

use super::UnwrapOrExit;

#[derive(Args)]
pub struct RenameCommand {
    #[command(subcommand)]
    command: RenameSubcommand,
}

#[derive(Subcommand)]
enum RenameSubcommand {
    /// Rename a command in bao.toml
    Command(RenameCommandArgs),
}

#[derive(Args)]
struct RenameCommandArgs {
    /// Current command name (use / for subcommands, e.g., "users/create")
    old_name: String,

    /// New command name (must be at same level, e.g., "users/add")
    new_name: String,

    /// Path to bao.toml
    #[arg(short, long, default_value = "bao.toml")]
    config: PathBuf,

    /// Output directory containing src/handlers
    #[arg(short, long, default_value = ".")]
    output: PathBuf,
}

impl RenameCommand {
    pub fn run(&self) -> Result<()> {
        match &self.command {
            RenameSubcommand::Command(args) => Self::rename_command(args),
        }
    }

    fn rename_command(args: &RenameCommandArgs) -> Result<()> {
        // Extract parent and leaf names
        let (old_parent, _old_leaf) = extract_parent_and_name(&args.old_name);
        let (new_parent, new_leaf) = extract_parent_and_name(&args.new_name);

        // Validate same parent level
        if old_parent != new_parent {
            eyre::bail!(
                "Cannot move command to different parent. \
                 '{}' and '{}' have different parents. \
                 Use `bao remove` and `bao add` instead.",
                args.old_name,
                args.new_name
            );
        }

        // Validate names are different
        if args.old_name == args.new_name {
            eyre::bail!("Old and new names are the same");
        }

        // Validate new name format
        if let Some(reason) = validate_command_name(new_leaf) {
            eyre::bail!("Invalid command name '{}': {}", new_leaf, reason);
        }

        // Open bao.toml
        let mut bao_toml = BaoToml::open(&args.config).unwrap_or_exit();

        // Validate old command exists
        if !bao_toml.schema().has_command(&args.old_name) {
            eyre::bail!("Command '{}' does not exist", args.old_name);
        }

        // Validate new command doesn't exist
        if bao_toml.schema().has_command(&args.new_name) {
            eyre::bail!("Command '{}' already exists", args.new_name);
        }

        // Update bao.toml
        let new_content =
            rename_command_section(bao_toml.content(), &args.old_name, &args.new_name);
        bao_toml.set_content(new_content)?;
        bao_toml.save()?;

        // Rename handler file/directory
        let renamed = rename_handler(&args.output, &args.old_name, &args.new_name)?;

        println!("Renamed command '{}' to '{}'", args.old_name, args.new_name);
        if let Some((old_path, new_path)) = renamed {
            println!("  {} -> {}", old_path.display(), new_path.display());
        }

        Ok(())
    }
}

/// Extract parent path and leaf name from a command path
fn extract_parent_and_name(path: &str) -> (Option<&str>, &str) {
    match path.rsplit_once('/') {
        Some((parent, name)) => (Some(parent), name),
        None => (None, path),
    }
}

/// Validate a command name (basic validation)
fn validate_command_name(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        return Some("name cannot be empty");
    }

    // Check reserved keywords
    const RUST_KEYWORDS: &[&str] = &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
        "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait",
        "true", "type", "unsafe", "use", "where", "while", "abstract", "become", "box", "do",
        "final", "macro", "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
    ];

    if RUST_KEYWORDS.contains(&name) {
        return Some("name is a Rust reserved keyword");
    }

    // Also check snake_case version
    let snake = name.replace('-', "_");
    if RUST_KEYWORDS.contains(&snake.as_str()) {
        return Some("name converts to a Rust reserved keyword");
    }

    // Check first character
    let mut chars = name.chars().peekable();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        Some(_) => return Some("name must start with a letter or underscore"),
        None => return Some("name cannot be empty"),
    }

    // Check remaining characters
    let mut prev_was_dash = false;
    for c in chars {
        if c == '-' {
            if prev_was_dash {
                return Some("name cannot contain consecutive dashes");
            }
            prev_was_dash = true;
        } else if c.is_ascii_alphanumeric() || c == '_' {
            prev_was_dash = false;
        } else {
            return Some("name must contain only letters, numbers, underscores, and dashes");
        }
    }

    if prev_was_dash {
        return Some("name cannot end with a dash");
    }

    None
}

/// Convert a command name to snake_case for file paths
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap());
    }
    result.replace('-', "_")
}

/// Rename handler file or directory
fn rename_handler(
    output: &Path,
    old_name: &str,
    new_name: &str,
) -> Result<Option<(PathBuf, PathBuf)>> {
    let handlers_dir = output.join("src/handlers");

    // Convert command paths to file paths
    let old_segments: Vec<String> = old_name.split('/').map(to_snake_case).collect();
    let new_segments: Vec<String> = new_name.split('/').map(to_snake_case).collect();

    // Build paths
    let old_file = handlers_dir.join(format!("{}.rs", old_segments.join("/")));
    let new_file = handlers_dir.join(format!("{}.rs", new_segments.join("/")));

    // Check for directory (parent command with subcommands)
    let old_dir = handlers_dir.join(old_segments.join("/"));
    let new_dir = handlers_dir.join(new_segments.join("/"));

    // Try to rename file first
    if old_file.exists() {
        std::fs::rename(&old_file, &new_file)
            .wrap_err_with(|| "Failed to rename handler file".to_string())?;
        return Ok(Some((old_file, new_file)));
    }

    // Try to rename directory (for parent commands)
    if old_dir.is_dir() {
        std::fs::rename(&old_dir, &new_dir)
            .wrap_err_with(|| "Failed to rename handler directory".to_string())?;
        return Ok(Some((old_dir, new_dir)));
    }

    // Handler doesn't exist (will be created on next bake)
    Ok(None)
}
