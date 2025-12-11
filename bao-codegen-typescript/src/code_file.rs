//! CodeFile abstraction for structured TypeScript file generation.
//!
//! Provides a high-level API for generating TypeScript files with
//! organized imports, body content, and exports sections.

use baobao_codegen::{CodeBuilder, CodeFragment, Indent, Renderable};

use crate::ast::{Export, Import};

/// A structured representation of a TypeScript file.
///
/// Organizes code into three sections: imports, body, and exports.
/// Each section is rendered in order with appropriate blank lines.
///
/// # Example
///
/// ```ignore
/// let file = CodeFile::new()
///     .import(Import::new("boune").named("defineCommand"))
///     .add(command_schema)
///     .export(Export::new().named("fooCommand"))
///     .render();
/// ```
#[derive(Default)]
pub struct CodeFile {
    imports: Vec<Import>,
    body: Vec<Vec<CodeFragment>>,
    exports: Vec<Export>,
}

impl CodeFile {
    /// Create a new empty CodeFile.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an import statement.
    pub fn import(mut self, import: Import) -> Self {
        self.imports.push(import);
        self
    }

    /// Add imports from an iterator.
    pub fn imports(mut self, imports: impl IntoIterator<Item = Import>) -> Self {
        self.imports.extend(imports);
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

    /// Add an export statement.
    pub fn export(mut self, export: Export) -> Self {
        self.exports.push(export);
        self
    }

    /// Add exports from an iterator.
    pub fn exports(mut self, exports: impl IntoIterator<Item = Export>) -> Self {
        self.exports.extend(exports);
        self
    }

    /// Render the file with TypeScript indentation (2 spaces).
    pub fn render(&self) -> String {
        self.render_with_indent(Indent::TYPESCRIPT)
    }

    /// Render the file with custom indentation.
    pub fn render_with_indent(&self, indent: Indent) -> String {
        let mut builder = CodeBuilder::new(indent);

        // 1. Render imports
        for import in &self.imports {
            builder.emit(import);
        }

        // 2. Blank line between imports and body
        if !self.imports.is_empty() && (!self.body.is_empty() || !self.exports.is_empty()) {
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

        // 4. Blank line before exports
        if !self.body.is_empty() && !self.exports.is_empty() {
            builder.push_blank();
        }

        // 5. Render exports
        for export in &self.exports {
            builder.emit(export);
        }

        builder.build()
    }

    /// Check if the file is empty.
    pub fn is_empty(&self) -> bool {
        self.imports.is_empty() && self.body.is_empty() && self.exports.is_empty()
    }
}

/// A raw code fragment that implements Renderable.
///
/// Useful for adding raw code strings to CodeFile body.
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
        let file = CodeFile::new();
        assert!(file.is_empty());
        assert_eq!(file.render(), "");
    }

    #[test]
    fn test_imports_only() {
        let file = CodeFile::new().import(Import::new("boune").named("defineCommand"));
        let code = file.render();
        assert!(code.contains("import { defineCommand } from \"boune\";"));
    }

    #[test]
    fn test_raw_code_body() {
        let file = CodeFile::new().add(RawCode::new("const x = 1;"));
        let code = file.render();
        assert_eq!(code, "const x = 1;\n");
    }

    #[test]
    fn test_full_file() {
        let file = CodeFile::new()
            .import(Import::new("boune").named("defineCommand"))
            .add(RawCode::new("const cmd = defineCommand({});"))
            .export(Export::new().named("cmd"));

        let code = file.render();
        assert!(code.contains("import { defineCommand }"));
        assert!(code.contains("const cmd = defineCommand"));
        assert!(code.contains("export { cmd }"));
    }

    #[test]
    fn test_blank_lines_between_body() {
        let file = CodeFile::new()
            .add(RawCode::new("const a = 1;"))
            .add(RawCode::new("const b = 2;"));

        let code = file.render();
        assert!(code.contains("const a = 1;\n\nconst b = 2;"));
    }
}
