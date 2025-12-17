//! TOML section manipulation utilities.
//!
//! This module provides utilities for editing bao.toml files at the
//! section level, preserving formatting and comments where possible.

/// Build a TOML section header for a command path.
///
/// # Examples
///
/// ```
/// use baobao_manifest::command_section_header;
///
/// assert_eq!(command_section_header("hello"), "[commands.hello]");
/// assert_eq!(command_section_header("users/create"), "[commands.users.commands.create]");
/// assert_eq!(command_section_header("db/migrate/up"), "[commands.db.commands.migrate.commands.up]");
/// ```
pub fn command_section_header(path: &str) -> String {
    if path.contains('/') {
        let parts: Vec<&str> = path.split('/').collect();
        let mut header = String::from("[commands");
        for part in &parts[..parts.len() - 1] {
            header.push('.');
            header.push_str(part);
            header.push_str(".commands");
        }
        header.push('.');
        header.push_str(parts.last().unwrap());
        header.push(']');
        header
    } else {
        format!("[commands.{}]", path)
    }
}

/// Build a TOML section header for a context field.
///
/// # Examples
///
/// ```
/// use baobao_manifest::context_section_header;
///
/// assert_eq!(context_section_header("database"), "[context.database]");
/// assert_eq!(context_section_header("http"), "[context.http]");
/// ```
pub fn context_section_header(name: &str) -> String {
    format!("[context.{}]", name)
}

/// Remove a TOML section and its content from a string.
///
/// This removes the section header and all content until the next section
/// (indicated by a line starting with `[`). Also cleans up extra blank lines.
///
/// # Arguments
///
/// * `content` - The full TOML content
/// * `section_header` - The exact section header to remove (e.g., "[commands.hello]")
///
/// # Examples
///
/// ```
/// use baobao_manifest::remove_toml_section;
///
/// let content = r#"[cli]
/// name = "myapp"
///
/// [commands.hello]
/// description = "Say hello"
///
/// [commands.world]
/// description = "Say world"
/// "#;
///
/// let result = remove_toml_section(content, "[commands.hello]");
/// assert!(!result.contains("[commands.hello]"));
/// assert!(result.contains("[commands.world]"));
/// ```
pub fn remove_toml_section(content: &str, section_header: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut skip = false;
    let mut skip_blank_after = false;

    for line in lines {
        if line.trim() == section_header {
            skip = true;
            skip_blank_after = true;
            continue;
        }

        if skip {
            // Stop skipping when we hit another section
            if line.starts_with('[') {
                skip = false;
                skip_blank_after = false;
            } else {
                continue;
            }
        }

        // Skip blank lines immediately after removed section
        if skip_blank_after && line.trim().is_empty() {
            skip_blank_after = false;
            continue;
        }

        result.push(line);
    }

    // Clean up trailing blank lines
    while result.last().is_some_and(|l| l.trim().is_empty()) {
        result.pop();
    }

    if result.is_empty() {
        String::new()
    } else {
        format!("{}\n", result.join("\n"))
    }
}

/// Rename a command in TOML content by replacing section headers.
///
/// This replaces the section header for the old command with the new one,
/// and also updates any nested subcommand section prefixes.
///
/// # Arguments
///
/// * `content` - The full TOML content
/// * `old_name` - The old command path (e.g., "users" or "users/create")
/// * `new_name` - The new command path (must be at same level)
///
/// # Examples
///
/// ```
/// use baobao_manifest::rename_command_section;
///
/// let content = r#"[commands.users]
/// description = "User management"
///
/// [commands.users.commands.create]
/// description = "Create user"
/// "#;
///
/// let result = rename_command_section(content, "users", "accounts");
/// assert!(result.contains("[commands.accounts]"));
/// assert!(result.contains("[commands.accounts.commands.create]"));
/// ```
pub fn rename_command_section(content: &str, old_name: &str, new_name: &str) -> String {
    let old_header = command_section_header(old_name);
    let new_header = command_section_header(new_name);

    // Replace the section header
    let mut result = content.replace(&old_header, &new_header);

    // For top-level commands, also replace nested section prefixes
    // e.g., renaming "users" -> "accounts" also updates [commands.users.commands.X]
    if !old_name.contains('/') {
        let old_prefix = format!("[commands.{}.", old_name);
        let new_prefix = format!("[commands.{}.", new_name);
        result = result.replace(&old_prefix, &new_prefix);
    }

    result
}

/// Append a section to TOML content with proper spacing.
///
/// # Arguments
///
/// * `content` - The current TOML content
/// * `section` - The section to append (including header and fields)
///
/// # Returns
///
/// The new content with the section appended.
pub fn append_section(content: &str, section: &str) -> String {
    format!("{}\n\n{}", content.trim_end(), section.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_section_header_simple() {
        assert_eq!(command_section_header("hello"), "[commands.hello]");
    }

    #[test]
    fn test_command_section_header_nested() {
        assert_eq!(
            command_section_header("users/create"),
            "[commands.users.commands.create]"
        );
    }

    #[test]
    fn test_command_section_header_deeply_nested() {
        assert_eq!(
            command_section_header("db/migrate/up"),
            "[commands.db.commands.migrate.commands.up]"
        );
    }

    #[test]
    fn test_context_section_header() {
        assert_eq!(context_section_header("database"), "[context.database]");
        assert_eq!(context_section_header("http"), "[context.http]");
    }

    #[test]
    fn test_remove_toml_section_basic() {
        let content = r#"[cli]
name = "myapp"

[commands.hello]
description = "Say hello"

[commands.world]
description = "Say world"
"#;

        let result = remove_toml_section(content, "[commands.hello]");
        assert!(!result.contains("[commands.hello]"));
        assert!(!result.contains("Say hello"));
        assert!(result.contains("[commands.world]"));
        assert!(result.contains("Say world"));
    }

    #[test]
    fn test_remove_toml_section_last() {
        let content = r#"[cli]
name = "myapp"

[commands.hello]
description = "Say hello"
"#;

        let result = remove_toml_section(content, "[commands.hello]");
        assert!(!result.contains("[commands.hello]"));
        assert!(result.contains("[cli]"));
    }

    #[test]
    fn test_rename_command_section() {
        let content = r#"[commands.users]
description = "User management"
"#;

        let result = rename_command_section(content, "users", "accounts");
        assert!(result.contains("[commands.accounts]"));
        assert!(!result.contains("[commands.users]"));
    }

    #[test]
    fn test_rename_command_section_with_nested() {
        let content = r#"[commands.users]
description = "User management"

[commands.users.commands.create]
description = "Create user"
"#;

        let result = rename_command_section(content, "users", "accounts");
        assert!(result.contains("[commands.accounts]"));
        assert!(result.contains("[commands.accounts.commands.create]"));
        assert!(!result.contains("[commands.users]"));
    }

    #[test]
    fn test_append_section() {
        let content = "[cli]\nname = \"myapp\"";
        let section = "[commands.hello]\ndescription = \"Hello\"";

        let result = append_section(content, section);
        assert!(result.contains("[cli]"));
        assert!(result.contains("[commands.hello]"));
        assert!(result.contains("\n\n")); // proper spacing
    }
}
