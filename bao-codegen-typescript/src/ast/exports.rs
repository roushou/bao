//! TypeScript export builder.

use baobao_codegen::CodeBuilder;

/// Builder for TypeScript export statements.
#[derive(Debug, Clone)]
pub struct Export {
    from: Option<String>,
    default: Option<String>,
    named: Vec<String>,
    type_only: bool,
}

impl Export {
    pub fn new() -> Self {
        Self {
            from: None,
            default: None,
            named: Vec::new(),
            type_only: false,
        }
    }

    /// Re-export from another module.
    pub fn from(mut self, module: impl Into<String>) -> Self {
        self.from = Some(module.into());
        self
    }

    /// Export as default.
    pub fn default(mut self, name: impl Into<String>) -> Self {
        self.default = Some(name.into());
        self
    }

    /// Export a named item.
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.named.push(name.into());
        self
    }

    /// Make this a type-only export (`export type { ... }`).
    pub fn type_only(mut self) -> Self {
        self.type_only = true;
        self
    }

    /// Render the export to a CodeBuilder.
    pub fn render(&self, builder: CodeBuilder) -> CodeBuilder {
        let type_kw = if self.type_only { "type " } else { "" };

        let export_str = match (&self.from, &self.default, self.named.is_empty()) {
            // Re-export all: export * from "module"
            (Some(from), None, true) => {
                format!("export * from \"{}\";", from)
            }
            // Re-export named: export { a, b } from "module"
            (Some(from), None, false) => {
                format!(
                    "export {}{{ {} }} from \"{}\";",
                    type_kw,
                    self.named.join(", "),
                    from
                )
            }
            // Export default: export default foo
            (None, Some(def), true) => {
                format!("export default {};", def)
            }
            // Export named: export { a, b }
            (None, None, false) => {
                format!("export {}{{ {} }};", type_kw, self.named.join(", "))
            }
            // Invalid combinations - return empty
            _ => String::new(),
        };

        if export_str.is_empty() {
            builder
        } else {
            builder.line(&export_str)
        }
    }

    /// Build the export as a string.
    pub fn build(&self) -> String {
        self.render(CodeBuilder::typescript()).build()
    }
}

impl Default for Export {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_default() {
        let e = Export::new().default("Foo").build();
        assert_eq!(e, "export default Foo;\n");
    }

    #[test]
    fn test_export_named() {
        let e = Export::new().named("foo").named("bar").build();
        assert_eq!(e, "export { foo, bar };\n");
    }

    #[test]
    fn test_re_export_all() {
        let e = Export::new().from("./module").build();
        assert_eq!(e, "export * from \"./module\";\n");
    }

    #[test]
    fn test_re_export_named() {
        let e = Export::new()
            .from("./utils")
            .named("foo")
            .named("bar")
            .build();
        assert_eq!(e, "export { foo, bar } from \"./utils\";\n");
    }

    #[test]
    fn test_export_type_only() {
        let e = Export::new()
            .named("Config")
            .named("Options")
            .type_only()
            .build();
        assert_eq!(e, "export type { Config, Options };\n");
    }

    #[test]
    fn test_re_export_type_only() {
        let e = Export::new()
            .from("./types")
            .named("User")
            .type_only()
            .build();
        assert_eq!(e, "export type { User } from \"./types\";\n");
    }
}
