//! Diagnostic types for the compilation pipeline.
//!
//! This module provides types for collecting errors, warnings, and informational
//! messages during compilation phases.

use serde::Serialize;

/// Severity level for a diagnostic message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Severity {
    /// A fatal error that prevents further processing.
    Error,
    /// A warning that doesn't prevent processing but should be addressed.
    Warning,
    /// Informational message about the compilation process.
    Info,
}

impl Severity {
    /// Returns true if this is an error severity.
    pub fn is_error(&self) -> bool {
        matches!(self, Severity::Error)
    }

    /// Returns true if this is a warning severity.
    pub fn is_warning(&self) -> bool {
        matches!(self, Severity::Warning)
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Info => write!(f, "info"),
        }
    }
}

/// A diagnostic message from a compilation phase.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    /// The severity level of this diagnostic.
    pub severity: Severity,
    /// The phase that produced this diagnostic.
    pub phase: String,
    /// The diagnostic message.
    pub message: String,
    /// Optional location in the manifest (e.g., "commands.deploy").
    pub location: Option<String>,
}

impl Diagnostic {
    /// Create a new error diagnostic.
    pub fn error(phase: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            phase: phase.into(),
            message: message.into(),
            location: None,
        }
    }

    /// Create a new warning diagnostic.
    pub fn warning(phase: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            phase: phase.into(),
            message: message.into(),
            location: None,
        }
    }

    /// Create a new info diagnostic.
    pub fn info(phase: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Info,
            phase: phase.into(),
            message: message.into(),
            location: None,
        }
    }

    /// Add a location to this diagnostic.
    pub fn at(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.severity, self.message)?;
        if let Some(loc) = &self.location {
            write!(f, " (at {})", loc)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_error() {
        let diag = Diagnostic::error("validate", "invalid command name");
        assert!(diag.severity.is_error());
        assert_eq!(diag.phase, "validate");
    }

    #[test]
    fn test_diagnostic_with_location() {
        let diag = Diagnostic::warning("validate", "missing description").at("commands.deploy");
        assert!(diag.location.is_some());
        assert_eq!(diag.location.as_deref(), Some("commands.deploy"));
    }

    #[test]
    fn test_severity_display() {
        assert_eq!(Severity::Error.to_string(), "error");
        assert_eq!(Severity::Warning.to_string(), "warning");
        assert_eq!(Severity::Info.to_string(), "info");
    }
}
