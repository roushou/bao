//! Rust struct builder.

use baobao_codegen::builder::{CodeBuilder, CodeFragment, Renderable};

/// A field in a Rust struct.
#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: String,
    pub doc: Option<String>,
    pub attrs: Vec<String>,
    pub is_public: bool,
}

impl Field {
    pub fn new(name: impl Into<String>, ty: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ty: ty.into(),
            doc: None,
            attrs: Vec::new(),
            is_public: true,
        }
    }

    pub fn doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
    }

    pub fn attr(mut self, attr: impl Into<String>) -> Self {
        self.attrs.push(attr.into());
        self
    }

    pub fn private(mut self) -> Self {
        self.is_public = false;
        self
    }
}

/// Builder for Rust structs.
#[derive(Debug, Clone)]
pub struct Struct {
    name: String,
    doc: Option<String>,
    derives: Vec<String>,
    attrs: Vec<String>,
    fields: Vec<Field>,
    is_public: bool,
}

impl Struct {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            doc: None,
            derives: Vec::new(),
            attrs: Vec::new(),
            fields: Vec::new(),
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

    pub fn field(mut self, field: Field) -> Self {
        self.fields.push(field);
        self
    }

    pub fn private(mut self) -> Self {
        self.is_public = false;
        self
    }

    /// Render the struct to a CodeBuilder.
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

        if self.fields.is_empty() {
            builder.line(&format!("{}struct {} {{}}", vis, self.name))
        } else {
            let builder = builder
                .line(&format!("{}struct {} {{", vis, self.name))
                .indent();
            self.render_fields(builder).dedent().line("}")
        }
    }

    fn render_fields(&self, builder: CodeBuilder) -> CodeBuilder {
        self.fields.iter().fold(builder, |b, field| {
            let vis = if field.is_public { "pub " } else { "" };

            let b = if let Some(doc) = &field.doc {
                b.rust_doc(doc)
            } else {
                b
            };

            let b = field
                .attrs
                .iter()
                .fold(b, |b, attr| b.line(&format!("#[{}]", attr)));

            b.line(&format!("{}{}: {},", vis, field.name, field.ty))
        })
    }

    /// Build the struct as a string.
    pub fn build(&self) -> String {
        self.render(CodeBuilder::rust()).build()
    }

    /// Convert fields to code fragments.
    fn fields_to_fragments(&self) -> Vec<CodeFragment> {
        self.fields
            .iter()
            .flat_map(|field| {
                let mut fragments = Vec::new();
                let vis = if field.is_public { "pub " } else { "" };

                if let Some(doc) = &field.doc {
                    fragments.push(CodeFragment::RustDoc(doc.clone()));
                }

                for attr in &field.attrs {
                    fragments.push(CodeFragment::Line(format!("#[{}]", attr)));
                }

                fragments.push(CodeFragment::Line(format!(
                    "{}{}: {},",
                    vis, field.name, field.ty
                )));

                fragments
            })
            .collect()
    }
}

impl Renderable for Struct {
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

        if self.fields.is_empty() {
            fragments.push(CodeFragment::Line(format!(
                "{}struct {} {{}}",
                vis, self.name
            )));
        } else {
            fragments.push(CodeFragment::Block {
                header: format!("{}struct {} {{", vis, self.name),
                body: self.fields_to_fragments(),
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
    fn test_empty_struct() {
        let s = Struct::new("Empty").build();
        assert_eq!(s, "pub struct Empty {}\n");
    }

    #[test]
    fn test_struct_with_derives() {
        let s = Struct::new("Foo").derive("Debug").derive("Clone").build();
        assert!(s.contains("#[derive(Debug, Clone)]"));
        assert!(s.contains("pub struct Foo {}"));
    }

    #[test]
    fn test_struct_with_fields() {
        let s = Struct::new("Person")
            .derive("Debug")
            .field(Field::new("name", "String"))
            .field(Field::new("age", "u32"))
            .build();
        assert!(s.contains("pub struct Person {"));
        assert!(s.contains("pub name: String,"));
        assert!(s.contains("pub age: u32,"));
    }

    #[test]
    fn test_struct_with_attrs() {
        let s = Struct::new("Cli")
            .derive("Parser")
            .attr("command(name = \"mycli\")")
            .attr("command(version = \"1.0\")")
            .build();
        assert!(s.contains("#[derive(Parser)]"));
        assert!(s.contains("#[command(name = \"mycli\")]"));
        assert!(s.contains("#[command(version = \"1.0\")]"));
    }

    #[test]
    fn test_field_with_attrs() {
        let s = Struct::new("Args")
            .derive("Args")
            .field(
                Field::new("verbose", "bool")
                    .doc("Enable verbose output")
                    .attr("arg(long, short)"),
            )
            .build();
        assert!(s.contains("/// Enable verbose output"));
        assert!(s.contains("#[arg(long, short)]"));
        assert!(s.contains("pub verbose: bool,"));
    }
}
