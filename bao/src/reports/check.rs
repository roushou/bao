//! Check command report data structures.

use std::path::PathBuf;

use super::output::{Output, Report};

/// Report data from manifest validation.
#[derive(Debug)]
pub struct CheckReport {
    /// Path to the config file.
    pub config_path: PathBuf,
    /// Error messages.
    pub errors: Vec<String>,
    /// Warning messages.
    pub warnings: Vec<String>,
    /// Info messages.
    pub infos: Vec<String>,
}

impl CheckReport {
    /// Whether the check passed (no errors).
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

impl Report for CheckReport {
    fn render(&self, out: &mut dyn Output) {
        // Print errors
        for error in &self.errors {
            out.warning(&format!("error: {}", error));
        }

        // Print warnings
        for warning in &self.warnings {
            out.warning(&format!("warning: {}", warning));
        }

        // Print infos
        for info in &self.infos {
            out.preformatted(&format!("info: {}", info));
        }

        if !self.warnings.is_empty() || !self.errors.is_empty() {
            out.newline();
        }

        if self.is_valid() {
            out.preformatted(&format!("✓ {} is valid", self.config_path.display()));
        }
    }
}
