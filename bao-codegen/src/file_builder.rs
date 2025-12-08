//! Composable file builder

use crate::{CodeBuilder, ImportCollector, Indent};

/// Composable file builder that combines import collection with code generation.
///
/// This struct provides a language-agnostic way to build source files that need
/// both imports and code. Language-specific crates can add rendering methods.
///
/// # Example
///
/// ```
/// use baobao_codegen::{FileBuilder, Indent};
///
/// let mut builder = FileBuilder::new(Indent::RUST);
/// builder.imports.add("std::io", "Read");
/// builder.code = builder.code.line("fn main() {}");
///
/// // Language-specific rendering would be added by extension
/// ```
#[derive(Debug, Clone)]
pub struct FileBuilder {
    /// Import collector for tracking dependencies
    pub imports: ImportCollector,
    /// Code builder for generating the file body
    pub code: CodeBuilder,
}

impl FileBuilder {
    /// Create a new FileBuilder with the specified indentation.
    pub fn new(indent: Indent) -> Self {
        Self {
            imports: ImportCollector::new(),
            code: CodeBuilder::new(indent),
        }
    }

    /// Create a new FileBuilder with Rust-style indentation (4 spaces).
    pub fn rust() -> Self {
        Self::new(Indent::RUST)
    }

    /// Create a new FileBuilder with TypeScript-style indentation (2 spaces).
    pub fn typescript() -> Self {
        Self::new(Indent::TYPESCRIPT)
    }

    /// Create a new FileBuilder with Go-style indentation (tabs).
    pub fn go() -> Self {
        Self::new(Indent::GO)
    }

    /// Add an import.
    pub fn add_import(mut self, module: &str, symbol: &str) -> Self {
        self.imports.add(module, symbol);
        self
    }

    /// Add a module import without specific symbols.
    pub fn add_module(mut self, module: &str) -> Self {
        self.imports.add_module(module);
        self
    }

    /// Apply a function to the code builder.
    pub fn with_code<F>(mut self, f: F) -> Self
    where
        F: FnOnce(CodeBuilder) -> CodeBuilder,
    {
        self.code = f(self.code);
        self
    }

    /// Check if imports are empty.
    pub fn has_imports(&self) -> bool {
        !self.imports.is_empty()
    }

    /// Consume and return the individual components.
    pub fn into_parts(self) -> (ImportCollector, CodeBuilder) {
        (self.imports, self.code)
    }
}

impl Default for FileBuilder {
    fn default() -> Self {
        Self::rust()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_builder_basic() {
        let builder = FileBuilder::rust()
            .add_import("std::io", "Read")
            .add_import("std::io", "Write")
            .with_code(|c| c.line("fn main() {}"));

        assert!(builder.imports.has_symbol("std::io", "Read"));
        assert!(builder.imports.has_symbol("std::io", "Write"));
        assert!(builder.code.as_str().contains("fn main()"));
    }

    #[test]
    fn test_file_builder_fluent() {
        let builder = FileBuilder::rust()
            .add_import("clap", "Parser")
            .add_import("clap", "Subcommand")
            .with_code(|c| c.line("#[derive(Parser)]").line("struct Cli {}"));

        assert!(builder.imports.has_module("clap"));
        assert!(builder.code.as_str().contains("#[derive(Parser)]"));
    }
}
