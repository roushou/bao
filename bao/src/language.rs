//! Unified language dispatch.
//!
//! Centralizes language-specific generator creation and metadata.

use baobao_codegen::{language::LanguageCodegen, pipeline::CompilationContext};
use baobao_codegen_rust::Generator as RustGenerator;
use baobao_codegen_typescript::Generator as TypeScriptGenerator;
use baobao_manifest::Language;

/// Language-specific support for code generation.
///
/// Provides metadata and generator creation for a target language.
pub struct LanguageSupport {
    language: Language,
    /// Subdirectory for generated code (e.g., "src/generated/").
    pub gen_subdir: &'static str,
    /// File extension with dot (e.g., ".rs").
    pub extension: &'static str,
}

impl LanguageSupport {
    /// Get language support for the given language.
    pub fn get(language: Language) -> Self {
        match language {
            Language::Rust => Self {
                language,
                gen_subdir: "src/generated/",
                extension: ".rs",
            },
            Language::TypeScript => Self {
                language,
                gen_subdir: "src/",
                extension: ".ts",
            },
        }
    }

    /// Create a generator for this language.
    pub fn generator(&self, ctx: CompilationContext) -> Box<dyn LanguageCodegen> {
        match self.language {
            Language::Rust => Box::new(RustGenerator::from_context(ctx)),
            Language::TypeScript => Box::new(TypeScriptGenerator::from_context(ctx)),
        }
    }
}
