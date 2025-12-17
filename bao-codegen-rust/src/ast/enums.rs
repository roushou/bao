//! Rust enum builder.

use baobao_codegen::builder::{CodeBuilder, CodeFragment, Renderable};

/// A variant in a Rust enum.
#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    pub doc: Option<String>,
    pub data: Option<String>,
    pub attrs: Vec<String>,
}

impl Variant {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            doc: None,
            data: None,
            attrs: Vec::new(),
        }
    }

    pub fn doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
    }

    /// Set tuple data for the variant, e.g., `Foo(Bar)`.
    pub fn tuple(mut self, data: impl Into<String>) -> Self {
        self.data = Some(data.into());
        self
    }

    /// Add an attribute to the variant, e.g., `value(name = "foo")`.
    pub fn attr(mut self, attr: impl Into<String>) -> Self {
        self.attrs.push(attr.into());
        self
    }
}

/// Builder for Rust enums.
#[derive(Debug, Clone)]
pub struct Enum {
    name: String,
    doc: Option<String>,
    derives: Vec<String>,
    attrs: Vec<String>,
    variants: Vec<Variant>,
    is_public: bool,
}

impl Enum {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            doc: None,
            derives: Vec::new(),
            attrs: Vec::new(),
            variants: Vec::new(),
            is_public: true,
        }
    }

    pub fn doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
    }

    pub fn derive(mut self, derive: impl Into<String>) -> Self {
        self.derives.push(derive.into());
        self
    }

    pub fn attr(mut self, attr: impl Into<String>) -> Self {
        self.attrs.push(attr.into());
        self
    }

    pub fn variant(mut self, variant: Variant) -> Self {
        self.variants.push(variant);
        self
    }

    pub fn private(mut self) -> Self {
        self.is_public = false;
        self
    }

    /// Render the enum to a CodeBuilder.
    pub fn render(&self, builder: CodeBuilder) -> CodeBuilder {
        let vis = if self.is_public { "pub " } else { "" };

        let builder = if let Some(doc) = &self.doc {
            builder.rust_doc(doc)
        } else {
            builder
        };

        let builder = if !self.derives.is_empty() {
            builder.line(&format!("#[derive({})]", self.derives.join(", ")))
        } else {
            builder
        };

        let builder = self
            .attrs
            .iter()
            .fold(builder, |b, attr| b.line(&format!("#[{}]", attr)));

        if self.variants.is_empty() {
            builder.line(&format!("{}enum {} {{}}", vis, self.name))
        } else {
            let builder = builder
                .line(&format!("{}enum {} {{", vis, self.name))
                .indent();
            self.render_variants(builder).dedent().line("}")
        }
    }

    fn render_variants(&self, builder: CodeBuilder) -> CodeBuilder {
        self.variants.iter().fold(builder, |b, variant| {
            let b = if let Some(doc) = &variant.doc {
                b.rust_doc(doc)
            } else {
                b
            };

            // Add variant attributes
            let b = variant
                .attrs
                .iter()
                .fold(b, |b, attr| b.line(&format!("#[{}]", attr)));

            let variant_str = match &variant.data {
                Some(data) => format!("{}({}),", variant.name, data),
                None => format!("{},", variant.name),
            };
            b.line(&variant_str)
        })
    }

    /// Build the enum as a string.
    pub fn build(&self) -> String {
        self.render(CodeBuilder::rust()).build()
    }

    /// Convert variants to code fragments.
    fn variants_to_fragments(&self) -> Vec<CodeFragment> {
        self.variants
            .iter()
            .flat_map(|variant| {
                let mut fragments = Vec::new();

                if let Some(doc) = &variant.doc {
                    fragments.push(CodeFragment::RustDoc(doc.clone()));
                }

                // Add variant attributes
                for attr in &variant.attrs {
                    fragments.push(CodeFragment::Line(format!("#[{}]", attr)));
                }

                let variant_str = match &variant.data {
                    Some(data) => format!("{}({}),", variant.name, data),
                    None => format!("{},", variant.name),
                };
                fragments.push(CodeFragment::Line(variant_str));

                fragments
            })
            .collect()
    }
}

impl Renderable for Enum {
    fn to_fragments(&self) -> Vec<CodeFragment> {
        let vis = if self.is_public { "pub " } else { "" };
        let mut fragments = Vec::new();

        if let Some(doc) = &self.doc {
            fragments.push(CodeFragment::RustDoc(doc.clone()));
        }

        if !self.derives.is_empty() {
            fragments.push(CodeFragment::Line(format!(
                "#[derive({})]",
                self.derives.join(", ")
            )));
        }

        for attr in &self.attrs {
            fragments.push(CodeFragment::Line(format!("#[{}]", attr)));
        }

        if self.variants.is_empty() {
            fragments.push(CodeFragment::Line(format!(
                "{}enum {} {{}}",
                vis, self.name
            )));
        } else {
            fragments.push(CodeFragment::Block {
                header: format!("{}enum {} {{", vis, self.name),
                body: self.variants_to_fragments(),
                close: Some("}".to_string()),
            });
        }

        fragments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_enum() {
        let e = Enum::new("Empty").build();
        assert_eq!(e, "pub enum Empty {}\n");
    }

    #[test]
    fn test_enum_with_derives() {
        let e = Enum::new("Status")
            .derive("Debug")
            .derive("Clone")
            .variant(Variant::new("Active"))
            .variant(Variant::new("Inactive"))
            .build();
        assert!(e.contains("#[derive(Debug, Clone)]"));
        assert!(e.contains("pub enum Status {"));
        assert!(e.contains("Active,"));
        assert!(e.contains("Inactive,"));
    }

    #[test]
    fn test_enum_with_tuple_variants() {
        let e = Enum::new("Commands")
            .derive("Subcommand")
            .variant(Variant::new("Add").tuple("AddArgs"))
            .variant(Variant::new("Remove").tuple("RemoveArgs"))
            .build();
        assert!(e.contains("Add(AddArgs),"));
        assert!(e.contains("Remove(RemoveArgs),"));
    }

    #[test]
    fn test_variant_with_doc() {
        let e = Enum::new("Commands")
            .variant(Variant::new("Foo").doc("Do the foo thing"))
            .build();
        assert!(e.contains("/// Do the foo thing"));
        assert!(e.contains("Foo,"));
    }
}
