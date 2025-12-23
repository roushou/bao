//! Lint for empty command descriptions.

use baobao_manifest::Manifest;

use super::super::Lint;
use crate::pipeline::Diagnostic;

/// Lint that warns about commands missing descriptions.
pub struct EmptyDescriptionLint;

impl Lint for EmptyDescriptionLint {
    fn name(&self) -> &'static str {
        "empty-description"
    }

    fn check(&self, manifest: &Manifest, diagnostics: &mut Vec<Diagnostic>) {
        for (name, cmd) in &manifest.commands {
            if cmd.description.is_empty() {
                diagnostics.push(
                    Diagnostic::warning(
                        "validate",
                        format!("command '{}' has no description", name),
                    )
                    .at(format!("commands.{}", name)),
                );
            }

            check_subcommand_descriptions(name, cmd, diagnostics);
        }
    }
}

fn check_subcommand_descriptions(
    parent_path: &str,
    cmd: &baobao_manifest::Command,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (name, subcmd) in &cmd.commands {
        let path = format!("{}.{}", parent_path, name);
        if subcmd.description.is_empty() {
            diagnostics.push(
                Diagnostic::warning("validate", format!("command '{}' has no description", path))
                    .at(format!("commands.{}", path)),
            );
        }
        check_subcommand_descriptions(&path, subcmd, diagnostics);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_manifest(content: &str) -> Manifest {
        toml::from_str(content).expect("Failed to parse test manifest")
    }

    #[test]
    fn test_empty_description() {
        let manifest = parse_manifest(
            r#"
            [cli]
            name = "test"
            language = "rust"

            [commands.deploy]
            description = ""
        "#,
        );

        let mut diagnostics = Vec::new();
        EmptyDescriptionLint.check(&manifest, &mut diagnostics);

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("deploy"));
        assert!(diagnostics[0].severity.is_warning());
    }

    #[test]
    fn test_has_description() {
        let manifest = parse_manifest(
            r#"
            [cli]
            name = "test"
            language = "rust"

            [commands.deploy]
            description = "Deploy the application"
        "#,
        );

        let mut diagnostics = Vec::new();
        EmptyDescriptionLint.check(&manifest, &mut diagnostics);

        assert!(diagnostics.is_empty());
    }
}
