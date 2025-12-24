//! Bake operation - code generation from manifest.

use std::path::Path;

use baobao_codegen::{
    pipeline::{Pipeline, Severity, SnapshotPlugin},
    schema::{CommandTree, DisplayStyle},
};
use baobao_manifest::Manifest;
use eyre::{Context, Result};

use crate::{
    language::LanguageSupport,
    reports::{
        BakeReport, GenerationResult, HandlerChanges, PreviewFile, PreviewResult, WrittenResult,
    },
};

/// Options for the bake operation.
pub struct BakeOptions<'a> {
    /// Output directory for generated code.
    pub output_dir: &'a Path,
    /// Whether to preview without writing files.
    pub dry_run: bool,
    /// Whether to output debug snapshots.
    pub visualize: bool,
}

/// Execute the bake operation.
///
/// Runs the pipeline on the manifest and generates code for the target language.
pub fn bake(manifest: &Manifest, lang: LanguageSupport, opts: BakeOptions) -> Result<BakeReport> {
    // Set up the pipeline with optional visualization
    let debug_dir = opts.output_dir.join(".bao/debug");
    let snapshot_plugin = if opts.visualize {
        Some(SnapshotPlugin::with_output_dir(&debug_dir))
    } else {
        None
    };

    // Run the pipeline to validate, lower, and analyze
    let mut pipeline = Pipeline::new();
    if let Some(plugin) = snapshot_plugin {
        pipeline = pipeline.plugin(plugin);
    }
    let ctx = pipeline.run(manifest.clone()).wrap_err("Pipeline failed")?;

    // Collect warnings
    let warnings: Vec<String> = ctx
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, Severity::Warning))
        .map(|d| d.message.clone())
        .collect();

    // Build command tree for display
    let tree = CommandTree::new(manifest);
    let command_count = tree.leaf_count();
    let command_tree = tree
        .display_style(DisplayStyle::WithSignature)
        .indent("  ")
        .to_string();

    // Generate code
    let generator = lang.generator(ctx);
    let result = if opts.dry_run {
        let files = generator
            .preview()
            .into_iter()
            .map(|f| PreviewFile {
                path: f.path,
                content: f.content,
            })
            .collect();
        GenerationResult::Preview(PreviewResult { files })
    } else {
        let gen_result = generator
            .generate(opts.output_dir)
            .wrap_err("Failed to generate code")?;

        GenerationResult::Written(WrittenResult {
            output_dir: opts.output_dir.to_path_buf(),
            gen_subdir: lang.gen_subdir.to_string(),
            handlers: HandlerChanges {
                created: gen_result.created_handlers,
                orphans: gen_result.orphan_handlers,
                extension: lang.extension.to_string(),
            },
            debug_dir: if opts.visualize {
                Some(debug_dir)
            } else {
                None
            },
        })
    };

    Ok(BakeReport {
        cli_name: manifest.cli.name.clone(),
        cli_version: manifest.cli.version.to_string(),
        cli_description: manifest.cli.description.clone(),
        warnings,
        command_count,
        command_tree,
        result,
    })
}
