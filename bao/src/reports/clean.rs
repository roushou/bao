//! Clean command report data structures.

use super::output::{Output, Report};

/// Report data from cleaning orphaned files.
#[derive(Debug)]
pub struct CleanReport {
    /// Whether this was a dry run.
    pub dry_run: bool,
    /// Warning messages from pipeline.
    pub warnings: Vec<String>,
    /// Deleted command files.
    pub deleted_commands: Vec<String>,
    /// Deleted handler files.
    pub deleted_handlers: Vec<String>,
    /// Skipped handler files (modified by user).
    pub skipped_handlers: Vec<String>,
}

impl CleanReport {
    /// Whether any files were deleted (or would be deleted in dry run).
    pub fn has_deletions(&self) -> bool {
        !self.deleted_commands.is_empty() || !self.deleted_handlers.is_empty()
    }

    /// Whether any files were skipped.
    pub fn has_skipped(&self) -> bool {
        !self.skipped_handlers.is_empty()
    }
}

impl Report for CleanReport {
    fn render(&self, out: &mut dyn Output) {
        // Print warnings
        for warning in &self.warnings {
            out.warning(warning);
        }

        if !self.has_deletions() && !self.has_skipped() {
            out.preformatted("No orphaned files found.");
            return;
        }

        if self.has_deletions() {
            if self.dry_run {
                out.section("Would delete");
            } else {
                out.section("Deleted");
            }
            for path in &self.deleted_commands {
                out.removed_item(path);
            }
            for path in &self.deleted_handlers {
                out.removed_item(path);
            }
        }

        if self.has_skipped() {
            out.newline();
            out.section("Skipped (modified by user)");
            for path in &self.skipped_handlers {
                out.list_item(&format!("! {}", path));
            }
        }
    }
}
