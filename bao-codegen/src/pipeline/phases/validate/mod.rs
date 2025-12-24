//! Validate phase - runs lints on the manifest.

mod lint;
pub mod lints;

use eyre::{Result, bail};
pub use lint::{Lint, LintInfo};
pub use lints::{CommandNamingLint, DuplicateCommandLint, EmptyDescriptionLint};

use crate::pipeline::{CompilationContext, Phase};

/// Phase that validates the manifest using configurable lints.
pub struct ValidatePhase {
    lints: Vec<Box<dyn Lint>>,
}

impl ValidatePhase {
    /// Create a new validate phase with default lints.
    pub fn new() -> Self {
        Self {
            lints: vec![
                Box::new(CommandNamingLint),
                Box::new(DuplicateCommandLint),
                Box::new(EmptyDescriptionLint),
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

impl ValidatePhase {
    /// Get the names of all lints that will be run.
    pub fn lint_names(&self) -> Vec<&'static str> {
        self.lints.iter().map(|l| l.name()).collect()
    }

    /// Get information about all lints that will be run.
    pub fn lint_info(&self) -> Vec<LintInfo> {
        self.lints.iter().map(|l| l.info()).collect()
    }
}

impl Phase for ValidatePhase {
    fn name(&self) -> &'static str {
        "validate"
    }

    fn description(&self) -> &'static str {
        "Check manifest integrity and collect diagnostics"
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

#[cfg(test)]
mod tests {
    use baobao_manifest::Manifest;

    use super::*;
    use crate::pipeline::Diagnostic;

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
    fn test_with_errors() {
        struct AlwaysErrorLint;
        impl Lint for AlwaysErrorLint {
            fn name(&self) -> &'static str {
                "always-error"
            }
            fn description(&self) -> &'static str {
                "Always produces an error"
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
    fn test_warnings_allowed() {
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

        // Only use EmptyDescriptionLint which produces warnings
        let phase = ValidatePhase::empty().with_lint(EmptyDescriptionLint);
        let result = phase.run(&mut ctx);

        // Warnings don't cause failure
        assert!(result.is_ok());
        assert!(ctx.has_warnings());
        assert!(!ctx.has_errors());
    }
}
