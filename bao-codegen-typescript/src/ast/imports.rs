//! TypeScript import builder.

use baobao_codegen::{CodeBuilder, CodeFragment, Renderable};

/// A named import item that can be either a value or type import.
#[derive(Debug, Clone)]
struct NamedImport {
    name: String,
    is_type: bool,
}

/// Builder for TypeScript import statements.
#[derive(Debug, Clone)]
pub struct Import {
    from: String,
    default: Option<String>,
    named: Vec<NamedImport>,
    type_only: bool,
}

impl Import {
    pub fn new(from: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            default: None,
            named: Vec::new(),
            type_only: false,
        }
    }

    /// Import a default export.
    pub fn default(mut self, name: impl Into<String>) -> Self {
        self.default = Some(name.into());
        self
    }

    /// Import a named export.
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.named.push(NamedImport {
            name: name.into(),
            is_type: false,
        });
        self
    }

    /// Import a named type export (inline `type` keyword).
    pub fn named_type(mut self, name: impl Into<String>) -> Self {
        self.named.push(NamedImport {
            name: name.into(),
            is_type: true,
        });
        self
    }

    /// Make this a type-only import (`import type { ... }`).
    pub fn type_only(mut self) -> Self {
        self.type_only = true;
        self
    }

    fn format_named_imports(&self) -> String {
        self.named
            .iter()
            .map(|n| {
                if n.is_type {
                    format!("type {}", n.name)
                } else {
                    n.name.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Render the import to a CodeBuilder.
    pub fn render(&self, builder: CodeBuilder) -> CodeBuilder {
        let type_kw = if self.type_only { "type " } else { "" };
        let named_str = self.format_named_imports();

        let import_str = match (&self.default, self.named.is_empty()) {
            (Some(def), true) => {
                format!("import {}{} from \"{}\";", type_kw, def, self.from)
            }
            (Some(def), false) => {
                format!(
                    "import {}{}, {{ {} }} from \"{}\";",
                    type_kw, def, named_str, self.from
                )
            }
            (None, false) => {
                format!(
                    "import {}{{ {} }} from \"{}\";",
                    type_kw, named_str, self.from
                )
            }
            (None, true) => {
                format!("import \"{}\";", self.from)
            }
        };

        builder.line(&import_str)
    }

    /// Build the import as a string.
    pub fn build(&self) -> String {
        self.render(CodeBuilder::typescript()).build()
    }

    /// Format the import statement as a string.
    fn format_import(&self) -> String {
        let type_kw = if self.type_only { "type " } else { "" };
        let named_str = self.format_named_imports();

        match (&self.default, self.named.is_empty()) {
            (Some(def), true) => {
                format!("import {}{} from \"{}\";", type_kw, def, self.from)
            }
            (Some(def), false) => {
                format!(
                    "import {}{}, {{ {} }} from \"{}\";",
                    type_kw, def, named_str, self.from
                )
            }
            (None, false) => {
                format!(
                    "import {}{{ {} }} from \"{}\";",
                    type_kw, named_str, self.from
                )
            }
            (None, true) => {
                format!("import \"{}\";", self.from)
            }
        }
    }
}

impl Renderable for Import {
    fn to_fragments(&self) -> Vec<CodeFragment> {
        vec![CodeFragment::Line(self.format_import())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_import() {
        let i = Import::new("./module").default("Foo").build();
        assert_eq!(i, "import Foo from \"./module\";\n");
    }

    #[test]
    fn test_named_import() {
        let i = Import::new("./utils").named("foo").named("bar").build();
        assert_eq!(i, "import { foo, bar } from \"./utils\";\n");
    }

    #[test]
    fn test_default_and_named_import() {
        let i = Import::new("react")
            .default("React")
            .named("useState")
            .named("useEffect")
            .build();
        assert_eq!(i, "import React, { useState, useEffect } from \"react\";\n");
    }

    #[test]
    fn test_type_only_import() {
        let i = Import::new("./types").named("Config").type_only().build();
        assert_eq!(i, "import type { Config } from \"./types\";\n");
    }

    #[test]
    fn test_side_effect_import() {
        let i = Import::new("./polyfill").build();
        assert_eq!(i, "import \"./polyfill\";\n");
    }
}
