//! Lint for command naming conventions.

use baobao_manifest::Manifest;

use super::super::Lint;
use crate::pipeline::Diagnostic;

/// Lint that warns about command names that aren't kebab-case.
///
/// Kebab-case means: lowercase letters, numbers, and hyphens only.
/// Examples: `deploy`, `run-migrations`, `db-migrate`
pub struct CommandNamingLint;

impl Lint for CommandNamingLint {
    fn name(&self) -> &'static str {
        "command-naming"
    }

    fn description(&self) -> &'static str {
        "Check command names follow kebab-case conventions"
    }

    fn check(&self, manifest: &Manifest, diagnostics: &mut Vec<Diagnostic>) {
        for (name, cmd) in &manifest.commands {
            check_command_name(name, name, diagnostics);
            check_subcommand_names(name, cmd, diagnostics);
        }
    }
}

fn is_kebab_case(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    // Must start with lowercase letter
    let mut chars = s.chars().peekable();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    // Rest must be lowercase letters, digits, or hyphens (no consecutive hyphens)
    let mut prev_hyphen = false;
    for c in chars {
        if c == '-' {
            if prev_hyphen {
                return false; // consecutive hyphens
            }
            prev_hyphen = true;
        } else if c.is_ascii_lowercase() || c.is_ascii_digit() {
            prev_hyphen = false;
        } else {
            return false; // invalid character
        }
    }
    // Must not end with hyphen
    !prev_hyphen
}

fn check_command_name(name: &str, path: &str, diagnostics: &mut Vec<Diagnostic>) {
    if !is_kebab_case(name) {
        diagnostics.push(
            Diagnostic::warning(
                "validate",
                format!(
                    "command '{}' should use kebab-case (e.g., 'my-command' not 'my_command' or 'myCommand')",
                    name
                ),
            )
            .at(format!("commands.{}", path)),
        );
    }
}

fn check_subcommand_names(
    parent_path: &str,
    cmd: &baobao_manifest::Command,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (name, subcmd) in &cmd.commands {
        let path = format!("{}.{}", parent_path, name);
        check_command_name(name, &path, diagnostics);
        check_subcommand_names(&path, subcmd, diagnostics);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_manifest(content: &str) -> Manifest {
        toml::from_str(content).expect("Failed to parse test manifest")
    }

    #[test]
    fn test_is_kebab_case_valid() {
        assert!(is_kebab_case("deploy"));
        assert!(is_kebab_case("run-migrations"));
        assert!(is_kebab_case("db-migrate"));
        assert!(is_kebab_case("a"));
        assert!(is_kebab_case("a1"));
        assert!(is_kebab_case("foo-bar-baz"));
        assert!(is_kebab_case("v2-api"));
    }

    #[test]
    fn test_is_kebab_case_invalid() {
        assert!(!is_kebab_case("")); // empty
        assert!(!is_kebab_case("Deploy")); // uppercase start
        assert!(!is_kebab_case("runMigrations")); // camelCase
        assert!(!is_kebab_case("run_migrations")); // snake_case
        assert!(!is_kebab_case("run--migrations")); // consecutive hyphens
        assert!(!is_kebab_case("-deploy")); // starts with hyphen
        assert!(!is_kebab_case("deploy-")); // ends with hyphen
        assert!(!is_kebab_case("1deploy")); // starts with digit
        assert!(!is_kebab_case("de ploy")); // contains space
    }

    #[test]
    fn test_valid_names() {
        let manifest = parse_manifest(
            r#"
            [cli]
            name = "test"
            language = "rust"

            [commands.deploy]
            description = "Deploy"

            [commands.run-migrations]
            description = "Run migrations"
        "#,
        );

        let mut diagnostics = Vec::new();
        CommandNamingLint.check(&manifest, &mut diagnostics);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_invalid_names() {
        let manifest = parse_manifest(
            r#"
            [cli]
            name = "test"
            language = "rust"

            [commands.runMigrations]
            description = "Run migrations"

            [commands.deploy_now]
            description = "Deploy now"
        "#,
        );

        let mut diagnostics = Vec::new();
        CommandNamingLint.check(&manifest, &mut diagnostics);

        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().all(|d| d.severity.is_warning()));
        assert!(
            diagnostics
                .iter()
                .any(|d| d.message.contains("runMigrations"))
        );
        assert!(diagnostics.iter().any(|d| d.message.contains("deploy_now")));
    }

    #[test]
    fn test_nested_invalid_name() {
        let manifest = parse_manifest(
            r#"
            [cli]
            name = "test"
            language = "rust"

            [commands.db]
            description = "Database commands"

            [commands.db.commands.runMigration]
            description = "Run migration"
        "#,
        );

        let mut diagnostics = Vec::new();
        CommandNamingLint.check(&manifest, &mut diagnostics);

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("runMigration"));
    }
}
