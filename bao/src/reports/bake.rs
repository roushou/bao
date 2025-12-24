//! Bake command report data structures.

use std::path::PathBuf;

use super::output::{Output, Report};

/// Report data from code generation.
#[derive(Debug)]
pub struct BakeReport {
    /// CLI name from manifest.
    pub cli_name: String,

    /// CLI version from manifest.
    pub cli_version: String,

    /// CLI description from manifest.
    pub cli_description: Option<String>,

    /// Warning messages from pipeline.
    pub warnings: Vec<String>,

    /// Number of leaf commands.
    pub command_count: usize,

    /// Command tree display string.
    pub command_tree: String,

    /// Generation result (files written or preview).
    pub result: GenerationResult,
}

/// Result of code generation.
#[derive(Debug)]
pub enum GenerationResult {
    /// Files were written to disk.
    Written(WrittenResult),
    /// Dry-run preview.
    Preview(PreviewResult),
}

/// Result when files were written to disk.
#[derive(Debug)]
pub struct WrittenResult {
    /// Output directory.
    pub output_dir: PathBuf,
    /// Generated code subdirectory (e.g., "src/generated/").
    pub gen_subdir: String,
    /// Handler file changes.
    pub handlers: HandlerChanges,
    /// Path to debug snapshots, if visualization was enabled.
    pub debug_dir: Option<PathBuf>,
}

/// Handler file changes.
#[derive(Debug, Default)]
pub struct HandlerChanges {
    /// Newly created handler files.
    pub created: Vec<String>,
    /// Orphaned handler files (no longer needed).
    pub orphans: Vec<String>,
    /// File extension for handlers.
    pub extension: String,
}

/// Result of a dry-run preview.
#[derive(Debug)]
pub struct PreviewResult {
    /// Files that would be generated.
    pub files: Vec<PreviewFile>,
}

/// A file in preview mode.
#[derive(Debug)]
pub struct PreviewFile {
    /// File path.
    pub path: String,
    /// File content.
    pub content: String,
}

impl Report for BakeReport {
    fn render(&self, out: &mut dyn Output) {
        match &self.result {
            GenerationResult::Written(written) => self.render_written(out, written),
            GenerationResult::Preview(preview) => self.render_preview(out, preview),
        }
    }
}

impl BakeReport {
    fn render_written(&self, out: &mut dyn Output, written: &WrittenResult) {
        // Print visualization info first if enabled
        if let Some(debug_dir) = &written.debug_dir {
            out.key_value(
                "Pipeline snapshots written to",
                &debug_dir.display().to_string(),
            );
            out.newline();
        }

        // Print warnings
        for warning in &self.warnings {
            out.warning(warning);
        }

        // Print header
        out.preformatted(&format!("{} v{}", self.cli_name, self.cli_version));
        if let Some(desc) = &self.cli_description {
            out.preformatted(desc);
        }
        out.newline();

        // Print commands
        out.section(&format!("Commands ({})", self.command_count));
        out.preformatted(&self.command_tree);
        out.newline();

        // Print generation summary
        out.key_value(
            "Generated",
            &format!("{}/{}", written.output_dir.display(), written.gen_subdir),
        );

        // Print handler changes
        self.render_handler_changes(out, &written.handlers);
    }

    fn render_handler_changes(&self, out: &mut dyn Output, handlers: &HandlerChanges) {
        if !handlers.created.is_empty() {
            out.newline();
            out.section("New handlers");
            for handler in &handlers.created {
                out.added_item(&format!("src/handlers/{}", handler));
            }
        }

        if !handlers.orphans.is_empty() {
            out.newline();
            out.section("Unused handlers");
            for orphan in &handlers.orphans {
                out.removed_item(&format!("src/handlers/{}{}", orphan, handlers.extension));
            }
        }
    }

    fn render_preview(&self, out: &mut dyn Output, preview: &PreviewResult) {
        for file in &preview.files {
            out.divider(&file.path);
            out.preformatted(&file.content);
        }

        out.divider("Summary");
        out.preformatted(&format!("{} files would be generated", preview.files.len()));
    }
}
