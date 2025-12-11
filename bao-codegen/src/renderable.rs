//! Renderable trait and CodeFragment for decoupled code generation.
//!
//! This module provides abstractions that allow AST nodes to be composed
//! and rendered without direct coupling to CodeBuilder.

/// Represents a fragment of generated code.
///
/// CodeFragments form an intermediate representation between AST nodes
/// and the final string output, enabling composition and transformation.
#[derive(Debug, Clone, PartialEq)]
pub enum CodeFragment {
    /// A single line of code (will have newline appended).
    Line(String),
    /// A blank line.
    Blank,
    /// Raw text without newline.
    Raw(String),
    /// A block with header, body fragments, and optional closing line.
    Block {
        header: String,
        body: Vec<CodeFragment>,
        close: Option<String>,
    },
    /// Indent the contained fragments.
    Indent(Vec<CodeFragment>),
    /// A sequence of fragments.
    Sequence(Vec<CodeFragment>),
    /// A JSDoc comment.
    JsDoc(String),
    /// A Rust doc comment.
    RustDoc(String),
}

impl CodeFragment {
    /// Create a line fragment.
    pub fn line(s: impl Into<String>) -> Self {
        Self::Line(s.into())
    }

    /// Create a blank line fragment.
    pub fn blank() -> Self {
        Self::Blank
    }

    /// Create a raw text fragment.
    pub fn raw(s: impl Into<String>) -> Self {
        Self::Raw(s.into())
    }

    /// Create a block fragment.
    pub fn block(
        header: impl Into<String>,
        body: Vec<CodeFragment>,
        close: Option<String>,
    ) -> Self {
        Self::Block {
            header: header.into(),
            body,
            close,
        }
    }

    /// Create an indented fragment sequence.
    pub fn indent(fragments: Vec<CodeFragment>) -> Self {
        Self::Indent(fragments)
    }

    /// Create a sequence of fragments.
    pub fn sequence(fragments: Vec<CodeFragment>) -> Self {
        Self::Sequence(fragments)
    }

    /// Create a JSDoc comment fragment.
    pub fn jsdoc(s: impl Into<String>) -> Self {
        Self::JsDoc(s.into())
    }

    /// Create a Rust doc comment fragment.
    pub fn rust_doc(s: impl Into<String>) -> Self {
        Self::RustDoc(s.into())
    }
}

/// Trait for types that can be rendered to code fragments.
///
/// Implement this trait for AST nodes to enable them to be rendered
/// through CodeBuilder without direct coupling.
pub trait Renderable {
    /// Convert this node to a sequence of code fragments.
    fn to_fragments(&self) -> Vec<CodeFragment>;
}

/// Blanket implementation for references.
impl<T: Renderable + ?Sized> Renderable for &T {
    fn to_fragments(&self) -> Vec<CodeFragment> {
        (*self).to_fragments()
    }
}

/// Blanket implementation for Box.
impl<T: Renderable + ?Sized> Renderable for Box<T> {
    fn to_fragments(&self) -> Vec<CodeFragment> {
        self.as_ref().to_fragments()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_fragment_constructors() {
        assert_eq!(
            CodeFragment::line("test"),
            CodeFragment::Line("test".to_string())
        );
        assert_eq!(CodeFragment::blank(), CodeFragment::Blank);
        assert_eq!(
            CodeFragment::raw("raw"),
            CodeFragment::Raw("raw".to_string())
        );
    }

    #[test]
    fn test_block_fragment() {
        let block = CodeFragment::block(
            "if (true) {",
            vec![CodeFragment::line("return 1;")],
            Some("}".to_string()),
        );
        match block {
            CodeFragment::Block {
                header,
                body,
                close,
            } => {
                assert_eq!(header, "if (true) {");
                assert_eq!(body.len(), 1);
                assert_eq!(close, Some("}".to_string()));
            }
            _ => panic!("Expected Block variant"),
        }
    }
}
