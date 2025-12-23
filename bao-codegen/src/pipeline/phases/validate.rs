//! Validate phase - runs lints on the manifest.

use baobao_manifest::Manifest;
use eyre::{Result, bail};

use crate::pipeline::{CompilationContext, Diagnostic, Phase};

/// A lint that checks the manifest for issues.
pub trait Lint: Send + Sync {
    /// The name of this lint.
    fn name(&self) -> &'static str;

    /// Check the manifest and add any diagnostics.
    fn check(&self, manifest: &Manifest, diagnostics: &mut Vec<Diagnostic>);
}

/// Phase that validates the manifest using configurable lints.
pub struct ValidatePhase {
    lints: Vec<Box<dyn Lint>>,
}

impl ValidatePhase {
    /// Create a new validate phase with default lints.
    pub fn new() -> Self {
        Self {
            lints: vec![
                Box::new(EmptyDescriptionLint),
                // Add more built-in lints here
            ],
        }
    }

    /// Create a validate phase with no lints.
    pub fn empty() -> Self {
        Self { lints: Vec::new() }
    }

    /// Add a custom lint to the validation phase.
    pub fn with_lint(mut self, lint: impl Lint + 'static) -> Self {
        self.lints.push(Box::new(lint));
        self
    }
}

impl Default for ValidatePhase {
    fn default() -> Self {
        Self::new()
    }
}

impl Phase for ValidatePhase {
    fn name(&self) -> &'static str {
        "validate"
    }

    fn run(&self, ctx: &mut CompilationContext) -> Result<()> {
        // Run all lints
        for lint in &self.lints {
            lint.check(&ctx.manifest, &mut ctx.diagnostics);
        }

        // Fail if there are any errors (warnings are allowed)
        if ctx.has_errors() {
            bail!("Validation failed with {} error(s)", ctx.error_count());
        }

        Ok(())
    }
}

// ============================================================================
// Built-in lints
// ============================================================================

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

            // Check subcommands recursively
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

    fn make_test_manifest() -> Manifest {
        parse_manifest(
            r#"
            [cli]
            name = "test"
            language = "rust"
        "#,
        )
    }

    #[test]
    fn test_empty_description_lint() {
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
    fn test_validate_phase_with_errors() {
        struct AlwaysErrorLint;
        impl Lint for AlwaysErrorLint {
            fn name(&self) -> &'static str {
                "always-error"
            }
            fn check(&self, _manifest: &Manifest, diagnostics: &mut Vec<Diagnostic>) {
                diagnostics.push(Diagnostic::error("test", "forced error"));
            }
        }

        let manifest = make_test_manifest();
        let mut ctx = CompilationContext::new(manifest);

        let phase = ValidatePhase::empty().with_lint(AlwaysErrorLint);
        let result = phase.run(&mut ctx);

        assert!(result.is_err());
        assert!(ctx.has_errors());
    }

    #[test]
    fn test_validate_phase_warnings_allowed() {
        let manifest = parse_manifest(
            r#"
            [cli]
            name = "test"
            language = "rust"

            [commands.deploy]
            description = ""
        "#,
        );

        let mut ctx = CompilationContext::new(manifest);

        let phase = ValidatePhase::new();
        let result = phase.run(&mut ctx);

        // Warnings don't cause failure
        assert!(result.is_ok());
        assert!(ctx.has_warnings());
        assert!(!ctx.has_errors());
    }
}
