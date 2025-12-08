//! Code builder utility for generating properly indented code.

use crate::Indent;

/// Fluent API for building code with proper indentation.
///
/// # Example
///
/// ```
/// use baobao_codegen::CodeBuilder;
///
/// let code = CodeBuilder::new(Default::default())
///     .line("fn main() {")
///     .indent()
///     .line("println!(\"Hello, world!\");")
///     .dedent()
///     .line("}")
///     .build();
///
/// assert_eq!(code, "fn main() {\n    println!(\"Hello, world!\");\n}\n");
/// ```
#[derive(Debug, Clone)]
pub struct CodeBuilder {
    indent_level: usize,
    indent: Indent,
    buffer: String,
}

impl CodeBuilder {
    /// Create a new CodeBuilder with the specified indentation.
    pub fn new(indent: Indent) -> Self {
        Self {
            indent_level: 0,
            indent,
            buffer: String::new(),
        }
    }

    /// Create a new CodeBuilder with 4-space indentation (Rust default).
    pub fn rust() -> Self {
        Self::new(Indent::RUST)
    }

    /// Create a new CodeBuilder with 2-space indentation (JS/TS default).
    pub fn typescript() -> Self {
        Self::new(Indent::TYPESCRIPT)
    }

    /// Create a new CodeBuilder with tab indentation (Go default).
    pub fn go() -> Self {
        Self::new(Indent::GO)
    }

    /// Add a line of code with current indentation.
    pub fn line(mut self, s: &str) -> Self {
        self.write_indent();
        self.buffer.push_str(s);
        self.buffer.push('\n');
        self
    }

    /// Add a blank line (no indentation).
    pub fn blank(mut self) -> Self {
        self.buffer.push('\n');
        self
    }

    /// Add raw text without indentation or newline.
    pub fn raw(mut self, s: &str) -> Self {
        self.buffer.push_str(s);
        self
    }

    /// Increase indentation level.
    pub fn indent(mut self) -> Self {
        self.indent_level += 1;
        self
    }

    /// Decrease indentation level.
    pub fn dedent(mut self) -> Self {
        self.indent_level = self.indent_level.saturating_sub(1);
        self
    }

    /// Add a block with automatic indentation.
    ///
    /// # Example
    ///
    /// ```
    /// use baobao_codegen::CodeBuilder;
    ///
    /// let code = CodeBuilder::rust()
    ///     .block("impl Foo {", |b| {
    ///         b.line("fn bar(&self) {}")
    ///     })
    ///     .build();
    /// ```
    pub fn block<F>(self, header: &str, f: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        let builder = self.line(header).indent();
        f(builder).dedent()
    }

    /// Add a block with a closing line.
    ///
    /// # Example
    ///
    /// ```
    /// use baobao_codegen::CodeBuilder;
    ///
    /// let code = CodeBuilder::rust()
    ///     .block_with_close("fn main() {", "}", |b| {
    ///         b.line("println!(\"Hello\");")
    ///     })
    ///     .build();
    /// ```
    pub fn block_with_close<F>(self, header: &str, close: &str, f: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        let builder = self.line(header).indent();
        f(builder).dedent().line(close)
    }

    /// Add a doc comment line (e.g., `/// text` for Rust).
    pub fn doc(mut self, prefix: &str, text: &str) -> Self {
        self.write_indent();
        self.buffer.push_str(prefix);
        self.buffer.push(' ');
        self.buffer.push_str(text);
        self.buffer.push('\n');
        self
    }

    /// Add a Rust doc comment (`/// text`).
    pub fn rust_doc(self, text: &str) -> Self {
        self.doc("///", text)
    }

    /// Add a JSDoc/TSDoc comment (`/** text */` for single line).
    pub fn jsdoc(mut self, text: &str) -> Self {
        self.write_indent();
        self.buffer.push_str("/** ");
        self.buffer.push_str(text);
        self.buffer.push_str(" */\n");
        self
    }

    /// Conditionally add content.
    pub fn when<F>(self, condition: bool, f: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        if condition { f(self) } else { self }
    }

    /// Iterate and add content for each item.
    pub fn each<T, I, F>(mut self, items: I, f: F) -> Self
    where
        I: IntoIterator<Item = T>,
        F: Fn(Self, T) -> Self,
    {
        for item in items {
            self = f(self, item);
        }
        self
    }

    /// Get the current indentation level.
    pub fn current_indent(&self) -> usize {
        self.indent_level
    }

    /// Consume the builder and return the generated code.
    pub fn build(self) -> String {
        self.buffer
    }

    /// Get a reference to the current buffer content.
    pub fn as_str(&self) -> &str {
        &self.buffer
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent_level {
            self.buffer.push_str(self.indent.as_str());
        }
    }
}

impl Default for CodeBuilder {
    fn default() -> Self {
        Self::rust()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_line() {
        let code = CodeBuilder::rust().line("let x = 1;").build();
        assert_eq!(code, "let x = 1;\n");
    }

    #[test]
    fn test_indentation() {
        let code = CodeBuilder::rust()
            .line("fn main() {")
            .indent()
            .line("println!(\"Hello\");")
            .dedent()
            .line("}")
            .build();

        assert_eq!(code, "fn main() {\n    println!(\"Hello\");\n}\n");
    }

    #[test]
    fn test_block() {
        let code = CodeBuilder::rust()
            .block_with_close("impl Foo {", "}", |b| b.line("fn bar(&self) {}"))
            .build();

        assert_eq!(code, "impl Foo {\n    fn bar(&self) {}\n}\n");
    }

    #[test]
    fn test_blank_line() {
        let code = CodeBuilder::rust()
            .line("use std::io;")
            .blank()
            .line("fn main() {}")
            .build();

        assert_eq!(code, "use std::io;\n\nfn main() {}\n");
    }

    #[test]
    fn test_doc_comment() {
        let code = CodeBuilder::rust()
            .rust_doc("A test function")
            .line("fn test() {}")
            .build();

        assert_eq!(code, "/// A test function\nfn test() {}\n");
    }

    #[test]
    fn test_conditional() {
        let with_debug = CodeBuilder::rust()
            .when(true, |b| b.line("#[derive(Debug)]"))
            .line("struct Foo;")
            .build();

        let without_debug = CodeBuilder::rust()
            .when(false, |b| b.line("#[derive(Debug)]"))
            .line("struct Foo;")
            .build();

        assert_eq!(with_debug, "#[derive(Debug)]\nstruct Foo;\n");
        assert_eq!(without_debug, "struct Foo;\n");
    }

    #[test]
    fn test_each() {
        let code = CodeBuilder::rust()
            .line("enum Color {")
            .indent()
            .each(["Red", "Green", "Blue"], |b, color| {
                b.line(&format!("{},", color))
            })
            .dedent()
            .line("}")
            .build();

        assert_eq!(code, "enum Color {\n    Red,\n    Green,\n    Blue,\n}\n");
    }

    #[test]
    fn test_typescript_indent() {
        let code = CodeBuilder::typescript()
            .line("function foo() {")
            .indent()
            .line("return 1;")
            .dedent()
            .line("}")
            .build();

        assert_eq!(code, "function foo() {\n  return 1;\n}\n");
    }
}
