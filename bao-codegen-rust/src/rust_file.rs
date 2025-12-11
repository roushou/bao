//! RustFile abstraction for structured Rust file generation.
//!
//! Provides a high-level API for generating Rust files with
//! organized imports and body content.

use baobao_codegen::{CodeBuilder, CodeFragment, Indent, Renderable};

/// A Rust use statement.
#[derive(Debug, Clone)]
pub struct Use {
    module: String,
    symbols: Vec<String>,
}

impl Use {
    /// Create a use statement for a module.
    pub fn new(module: impl Into<String>) -> Self {
        Self {
            module: module.into(),
            symbols: Vec::new(),
        }
    }

    /// Add a symbol to import from the module.
    pub fn symbol(mut self, symbol: impl Into<String>) -> Self {
        self.symbols.push(symbol.into());
        self
    }

    /// Add multiple symbols to import.
    pub fn symbols(mut self, symbols: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.symbols.extend(symbols.into_iter().map(Into::into));
        self
    }

    /// Format the use statement as a string.
    fn format(&self) -> String {
        if self.symbols.is_empty() {
            format!("use {};", self.module)
        } else if self.symbols.len() == 1 {
            format!("use {}::{};", self.module, self.symbols[0])
        } else {
            format!("use {}::{{{}}};", self.module, self.symbols.join(", "))
        }
    }
}

impl Renderable for Use {
    fn to_fragments(&self) -> Vec<CodeFragment> {
        vec![CodeFragment::Line(self.format())]
    }
}

/// A structured representation of a Rust file.
///
/// Organizes code into imports and body sections.
///
/// # Example
///
/// ```ignore
/// let file = RustFile::new()
///     .use_stmt(Use::new("clap").symbol("Parser"))
///     .add(my_struct)
///     .add(my_impl)
///     .render();
/// ```
#[derive(Default)]
pub struct RustFile {
    uses: Vec<Use>,
    body: Vec<Vec<CodeFragment>>,
}

impl RustFile {
    /// Create a new empty RustFile.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a use statement.
    pub fn use_stmt(mut self, use_stmt: Use) -> Self {
        self.uses.push(use_stmt);
        self
    }

    /// Add multiple use statements.
    pub fn use_stmts(mut self, uses: impl IntoIterator<Item = Use>) -> Self {
        self.uses.extend(uses);
        self
    }

    /// Add a body element (any Renderable).
    #[allow(clippy::should_implement_trait)]
    pub fn add<R: Renderable>(mut self, node: R) -> Self {
        self.body.push(node.to_fragments());
        self
    }

    /// Add multiple body elements.
    pub fn add_all<R: Renderable>(mut self, nodes: impl IntoIterator<Item = R>) -> Self {
        for node in nodes {
            self.body.push(node.to_fragments());
        }
        self
    }

    /// Render the file with Rust indentation (4 spaces).
    pub fn render(&self) -> String {
        self.render_with_indent(Indent::RUST)
    }

    /// Render the file with a header comment.
    pub fn render_with_header(&self, header: &str) -> String {
        let content = self.render();
        if content.is_empty() {
            format!("{}\n", header)
        } else {
            format!("{}\n\n{}", header, content)
        }
    }

    /// Render the file with custom indentation.
    pub fn render_with_indent(&self, indent: Indent) -> String {
        let mut builder = CodeBuilder::new(indent);

        // 1. Render use statements
        for use_stmt in &self.uses {
            builder.emit(use_stmt);
        }

        // 2. Blank line between uses and body
        if !self.uses.is_empty() && !self.body.is_empty() {
            builder.push_blank();
        }

        // 3. Render body with blank lines between elements
        for (i, fragments) in self.body.iter().enumerate() {
            if i > 0 {
                builder.push_blank();
            }
            for fragment in fragments {
                builder.apply_fragment(fragment.clone());
            }
        }

        builder.build()
    }

    /// Check if the file is empty.
    pub fn is_empty(&self) -> bool {
        self.uses.is_empty() && self.body.is_empty()
    }
}

/// A raw code fragment that implements Renderable.
///
/// Useful for adding raw code strings to RustFile body.
#[derive(Debug, Clone)]
pub struct RawCode(String);

impl RawCode {
    /// Create a new raw code fragment.
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    /// Create a raw code fragment from multiple lines.
    pub fn lines(lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self(
            lines
                .into_iter()
                .map(Into::into)
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }
}

impl Renderable for RawCode {
    fn to_fragments(&self) -> Vec<CodeFragment> {
        self.0
            .lines()
            .map(|line| CodeFragment::Line(line.to_string()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_file() {
        let file = RustFile::new();
        assert!(file.is_empty());
        assert_eq!(file.render(), "");
    }

    #[test]
    fn test_use_single_symbol() {
        let use_stmt = Use::new("clap").symbol("Parser");
        let mut builder = CodeBuilder::rust();
        builder.emit(&use_stmt);
        assert_eq!(builder.build(), "use clap::Parser;\n");
    }

    #[test]
    fn test_use_multiple_symbols() {
        let use_stmt = Use::new("std::collections").symbols(["HashMap", "HashSet"]);
        let mut builder = CodeBuilder::rust();
        builder.emit(&use_stmt);
        assert_eq!(
            builder.build(),
            "use std::collections::{HashMap, HashSet};\n"
        );
    }

    #[test]
    fn test_uses_only() {
        let file = RustFile::new().use_stmt(Use::new("clap").symbol("Parser"));
        let code = file.render();
        assert_eq!(code, "use clap::Parser;\n");
    }

    #[test]
    fn test_raw_code_body() {
        let file = RustFile::new().add(RawCode::new("fn main() {}"));
        let code = file.render();
        assert_eq!(code, "fn main() {}\n");
    }

    #[test]
    fn test_full_file() {
        let file = RustFile::new()
            .use_stmt(Use::new("clap").symbol("Parser"))
            .add(RawCode::new("fn main() {}"));

        let code = file.render();
        assert!(code.contains("use clap::Parser;"));
        assert!(code.contains("fn main() {}"));
    }

    #[test]
    fn test_blank_lines_between_body() {
        let file = RustFile::new()
            .add(RawCode::new("struct Foo;"))
            .add(RawCode::new("struct Bar;"));

        let code = file.render();
        assert!(code.contains("struct Foo;\n\nstruct Bar;"));
    }

    #[test]
    fn test_render_with_header() {
        let file = RustFile::new()
            .use_stmt(Use::new("clap").symbol("Parser"))
            .add(RawCode::new("fn main() {}"));

        let code = file.render_with_header("// Generated by Bao");
        assert!(code.starts_with("// Generated by Bao"));
        assert!(code.contains("use clap::Parser;"));
    }
}
