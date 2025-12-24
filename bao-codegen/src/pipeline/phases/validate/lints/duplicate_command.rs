//! Lint for duplicate command detection.

use std::collections::HashMap;

use baobao_manifest::Manifest;

use super::super::Lint;
use crate::pipeline::Diagnostic;

/// Lint that errors on duplicate command paths.
pub struct DuplicateCommandLint;

impl Lint for DuplicateCommandLint {
    fn name(&self) -> &'static str {
        "duplicate-command"
    }

    fn description(&self) -> &'static str {
        "Detect duplicate command names and paths"
    }

    fn check(&self, manifest: &Manifest, diagnostics: &mut Vec<Diagnostic>) {
        let mut seen: HashMap<String, String> = HashMap::new();

        for (name, cmd) in &manifest.commands {
            // Normalize to handle case-insensitive duplicates
            let normalized = name.to_lowercase();
            if let Some(first) = seen.get(&normalized) {
                diagnostics.push(
                    Diagnostic::error(
                        "validate",
                        format!("duplicate command '{}' (conflicts with '{}')", name, first),
                    )
                    .at(format!("commands.{}", name)),
                );
            } else {
                seen.insert(normalized, name.clone());
            }

            collect_subcommand_paths(name, cmd, &mut seen, diagnostics);
        }
    }
}

fn collect_subcommand_paths(
    parent_path: &str,
    cmd: &baobao_manifest::Command,
    seen: &mut HashMap<String, String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (name, subcmd) in &cmd.commands {
        let path = format!("{}/{}", parent_path, name);
        let normalized = path.to_lowercase();

        if let Some(first) = seen.get(&normalized) {
            diagnostics.push(
                Diagnostic::error(
                    "validate",
                    format!(
                        "duplicate command path '{}' (conflicts with '{}')",
                        path, first
                    ),
                )
                .at(format!("commands.{}", path.replace('/', "."))),
            );
        } else {
            seen.insert(normalized, path.clone());
        }

        collect_subcommand_paths(&path, subcmd, seen, diagnostics);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_manifest(content: &str) -> Manifest {
        toml::from_str(content).expect("Failed to parse test manifest")
    }

    #[test]
    fn test_no_duplicates() {
        let manifest = parse_manifest(
            r#"
            [cli]
            name = "test"
            language = "rust"

            [commands.deploy]
            description = "Deploy"

            [commands.build]
            description = "Build"
        "#,
        );

        let mut diagnostics = Vec::new();
        DuplicateCommandLint.check(&manifest, &mut diagnostics);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_nested_paths_distinct() {
        let manifest = parse_manifest(
            r#"
            [cli]
            name = "test"
            language = "rust"

            [commands.db]
            description = "Database commands"

            [commands.db.commands.migrate]
            description = "Migrate"

            [commands.api]
            description = "API commands"

            [commands.api.commands.migrate]
            description = "Another migrate"
        "#,
        );

        let mut diagnostics = Vec::new();
        DuplicateCommandLint.check(&manifest, &mut diagnostics);

        // db/migrate and api/migrate are different paths, so no duplicates
        assert!(diagnostics.is_empty());
    }
}
