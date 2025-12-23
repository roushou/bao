use std::path::PathBuf;

use baobao_codegen::{
    language::LanguageCodegen,
    pipeline::{Pipeline, Severity},
    schema::{CommandTree, CommandTreeExt, DisplayStyle},
};
use baobao_codegen_rust::Generator as RustGenerator;
use baobao_codegen_typescript::Generator as TypeScriptGenerator;
use baobao_manifest::{BaoToml, Language, Manifest};
use clap::Args;
use eyre::{Context, Result};

use super::UnwrapOrExit;

#[derive(Args)]
pub struct BakeCommand {
    /// Path to bao.toml (defaults to ./bao.toml)
    #[arg(short, long, default_value = "bao.toml")]
    pub config: PathBuf,

    /// Output directory (defaults to current directory)
    #[arg(short, long, default_value = ".")]
    pub output: PathBuf,

    /// Preview generated code without writing to disk
    #[arg(long)]
    pub dry_run: bool,

    /// Target language (overrides bao.toml setting)
    #[arg(short, long)]
    pub language: Option<Language>,
}

impl BakeCommand {
    /// Run the bake command
    pub fn run(&self) -> Result<()> {
        let bao_toml = BaoToml::open(&self.config).unwrap_or_exit();
        let schema = bao_toml.schema();

        // Use CLI flag if provided, otherwise use manifest setting
        let language = self.language.unwrap_or(schema.cli.language);

        // Run the pipeline to validate, lower, and analyze
        let pipeline = Pipeline::new();
        let ctx = pipeline.run(schema.clone()).wrap_err("Pipeline failed")?;

        // Print any warnings
        for diag in &ctx.diagnostics {
            if matches!(diag.severity, Severity::Warning) {
                eprintln!("warning: {}", diag.message);
            }
        }

        match language {
            Language::Rust => {
                let generator = RustGenerator::from_context(ctx);
                if self.dry_run {
                    self.run_preview(&generator)
                } else {
                    self.run_rust_generation(&generator, schema)
                }
            }
            Language::TypeScript => {
                let generator = TypeScriptGenerator::from_context(ctx);
                if self.dry_run {
                    self.run_preview(&generator)
                } else {
                    self.run_typescript_generation(&generator, schema)
                }
            }
        }
    }

    fn run_rust_generation(&self, generator: &RustGenerator, schema: &Manifest) -> Result<()> {
        let result = generator
            .generate(&self.output)
            .wrap_err("Failed to generate code")?;

        Self::print_generation_summary(schema, &self.output, "src/generated/", ".rs");
        Self::print_handler_changes(&result.created_handlers, &result.orphan_handlers, ".rs");

        Ok(())
    }

    fn run_typescript_generation(
        &self,
        generator: &TypeScriptGenerator,
        schema: &Manifest,
    ) -> Result<()> {
        let result = generator
            .generate(&self.output)
            .wrap_err("Failed to generate code")?;

        Self::print_generation_summary(schema, &self.output, "src/", ".ts");
        Self::print_handler_changes(&result.created_handlers, &result.orphan_handlers, ".ts");

        Ok(())
    }

    fn print_generation_summary(
        schema: &Manifest,
        output: &std::path::Path,
        gen_subdir: &str,
        _ext: &str,
    ) {
        // Print header
        println!("{} v{}", schema.cli.name, schema.cli.version);
        if let Some(desc) = &schema.cli.description {
            println!("{}", desc);
        }
        println!();

        // Print commands using declarative display
        let tree = CommandTree::new(schema);
        println!("Commands ({}):", tree.leaf_count());
        println!(
            "{}",
            tree.display_style(DisplayStyle::WithSignature).indent("  ")
        );
        println!();

        // Print generation summary
        println!("Generated: {}/{}", output.display(), gen_subdir);
    }

    fn print_handler_changes(created: &[String], orphans: &[String], ext: &str) {
        if !created.is_empty() {
            println!();
            println!("New handlers:");
            for handler in created {
                println!("  + src/handlers/{}", handler);
            }
        }

        if !orphans.is_empty() {
            println!();
            println!("Unused handlers:");
            for orphan in orphans {
                println!("  - src/handlers/{}{}", orphan, ext);
            }
        }
    }

    fn run_preview<G: LanguageCodegen>(&self, generator: &G) -> Result<()> {
        let files = generator.preview();

        for file in &files {
            println!("── {} ──", file.path);
            println!("{}", file.content);
        }

        println!("── Summary ──");
        println!("{} files would be generated", files.len());

        Ok(())
    }
}
