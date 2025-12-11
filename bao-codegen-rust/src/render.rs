//! Rust-specific rendering utilities for code generation.

use baobao_codegen::{builder::FileBuilder, generation::ImportCollector};

/// Render imports as Rust `use` statements.
///
/// # Example
///
/// ```
/// use baobao_codegen::generation::ImportCollector;
/// use baobao_codegen_rust::render_imports;
///
/// let mut imports = ImportCollector::new();
/// imports.add("std::collections", "HashMap");
/// imports.add("std::collections", "HashSet");
/// imports.add("std::io", "Read");
///
/// let rendered = render_imports(&imports);
/// assert!(rendered.contains("use std::collections::{HashMap, HashSet};"));
/// assert!(rendered.contains("use std::io::Read;"));
/// ```
pub fn render_imports(imports: &ImportCollector) -> String {
    let mut lines = Vec::new();
    for (module, symbols) in imports.iter() {
        if symbols.is_empty() {
            lines.push(format!("use {};", module));
        } else if symbols.len() == 1 {
            let symbol = symbols.iter().next().unwrap();
            lines.push(format!("use {}::{};", module, symbol));
        } else {
            let symbols: Vec<_> = symbols.iter().map(|s| s.as_str()).collect();
            lines.push(format!("use {}::{{{}}};", module, symbols.join(", ")));
        }
    }
    lines.join("\n")
}

/// Extension trait for FileBuilder with Rust-specific rendering.
pub trait RustFileBuilder {
    /// Render the file as Rust source code.
    ///
    /// Combines imports and code with proper spacing.
    fn render_rust(self) -> String;

    /// Render the file as Rust source code with a header.
    fn render_rust_with_header(self, header: &str) -> String;
}

impl RustFileBuilder for FileBuilder {
    fn render_rust(self) -> String {
        let (imports, code) = self.into_parts();
        let code_str = code.build();

        if imports.is_empty() {
            code_str
        } else {
            format!("{}\n\n{}", render_imports(&imports), code_str)
        }
    }

    fn render_rust_with_header(self, header: &str) -> String {
        let (imports, code) = self.into_parts();
        let code_str = code.build();

        if imports.is_empty() {
            format!("{}\n\n{}", header, code_str)
        } else {
            format!("{}\n\n{}\n\n{}", header, render_imports(&imports), code_str)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_imports_single() {
        let mut imports = ImportCollector::new();
        imports.add("std::io", "Read");

        let rendered = render_imports(&imports);
        assert_eq!(rendered, "use std::io::Read;");
    }

    #[test]
    fn test_render_imports_multiple_symbols() {
        let mut imports = ImportCollector::new();
        imports.add("std::io", "Read");
        imports.add("std::io", "Write");

        let rendered = render_imports(&imports);
        assert_eq!(rendered, "use std::io::{Read, Write};");
    }

    #[test]
    fn test_render_imports_multiple_modules() {
        let mut imports = ImportCollector::new();
        imports.add("std::io", "Read");
        imports.add("std::fs", "File");

        let rendered = render_imports(&imports);
        assert!(rendered.contains("use std::io::Read;"));
        assert!(rendered.contains("use std::fs::File;"));
    }

    #[test]
    fn test_file_builder_render_rust() {
        let builder = FileBuilder::rust()
            .add_import("clap", "Parser")
            .with_code(|c| c.line("fn main() {}"));

        let rendered = builder.render_rust();
        assert!(rendered.contains("use clap::Parser;"));
        assert!(rendered.contains("fn main() {}"));
    }

    #[test]
    fn test_file_builder_render_rust_with_header() {
        let builder = FileBuilder::rust()
            .add_import("clap", "Parser")
            .with_code(|c| c.line("fn main() {}"));

        let rendered = builder.render_rust_with_header("// Generated");
        assert!(rendered.starts_with("// Generated"));
        assert!(rendered.contains("use clap::Parser;"));
        assert!(rendered.contains("fn main() {}"));
    }

    #[test]
    fn test_file_builder_no_imports() {
        let builder = FileBuilder::rust().with_code(|c| c.line("fn main() {}"));

        let rendered = builder.render_rust();
        assert_eq!(rendered, "fn main() {}\n");
    }
}
