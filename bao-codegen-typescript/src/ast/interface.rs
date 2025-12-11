//! TypeScript interface builder.

use baobao_codegen::builder::{CodeBuilder, CodeFragment, Renderable};

/// A field in a TypeScript interface.
#[derive(Debug, Clone)]
pub struct InterfaceField {
    pub name: String,
    pub ty: String,
    pub optional: bool,
    pub readonly: bool,
}

impl InterfaceField {
    pub fn new(name: impl Into<String>, ty: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ty: ty.into(),
            optional: false,
            readonly: false,
        }
    }

    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    pub fn readonly(mut self) -> Self {
        self.readonly = true;
        self
    }
}

/// Builder for TypeScript interfaces.
#[derive(Debug, Clone)]
pub struct Interface {
    name: String,
    fields: Vec<InterfaceField>,
    exported: bool,
}

impl Interface {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            fields: Vec::new(),
            exported: true,
        }
    }

    /// Add a required field.
    pub fn field(mut self, name: impl Into<String>, ty: impl Into<String>) -> Self {
        self.fields.push(InterfaceField::new(name, ty));
        self
    }

    /// Add an optional field.
    pub fn optional_field(mut self, name: impl Into<String>, ty: impl Into<String>) -> Self {
        self.fields.push(InterfaceField::new(name, ty).optional());
        self
    }

    /// Add a field with full configuration.
    pub fn field_with(mut self, field: InterfaceField) -> Self {
        self.fields.push(field);
        self
    }

    /// Make this interface private (not exported).
    pub fn private(mut self) -> Self {
        self.exported = false;
        self
    }

    /// Render the interface to a CodeBuilder.
    pub fn render(&self, builder: CodeBuilder) -> CodeBuilder {
        let export = if self.exported { "export " } else { "" };

        if self.fields.is_empty() {
            builder.line(&format!("{}interface {} {{}}", export, self.name))
        } else {
            let builder = builder
                .line(&format!("{}interface {} {{", export, self.name))
                .indent();
            self.render_fields(builder).dedent().line("}")
        }
    }

    fn render_fields(&self, builder: CodeBuilder) -> CodeBuilder {
        self.fields.iter().fold(builder, |b, field| {
            let readonly = if field.readonly { "readonly " } else { "" };
            let optional = if field.optional { "?" } else { "" };
            b.line(&format!(
                "{}{}{}: {};",
                readonly, field.name, optional, field.ty
            ))
        })
    }

    /// Build the interface as a string.
    pub fn build(&self) -> String {
        self.render(CodeBuilder::typescript()).build()
    }

    /// Convert fields to code fragments.
    fn fields_to_fragments(&self) -> Vec<CodeFragment> {
        self.fields
            .iter()
            .map(|field| {
                let readonly = if field.readonly { "readonly " } else { "" };
                let optional = if field.optional { "?" } else { "" };
                CodeFragment::Line(format!(
                    "{}{}{}: {};",
                    readonly, field.name, optional, field.ty
                ))
            })
            .collect()
    }
}

impl Renderable for Interface {
    fn to_fragments(&self) -> Vec<CodeFragment> {
        let export = if self.exported { "export " } else { "" };

        if self.fields.is_empty() {
            vec![CodeFragment::Line(format!(
                "{}interface {} {{}}",
                export, self.name
            ))]
        } else {
            vec![CodeFragment::Block {
                header: format!("{}interface {} {{", export, self.name),
                body: self.fields_to_fragments(),
                close: Some("}".to_string()),
            }]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_interface() {
        let i = Interface::new("Empty").build();
        assert_eq!(i, "export interface Empty {}\n");
    }

    #[test]
    fn test_interface_with_fields() {
        let i = Interface::new("Person")
            .field("name", "string")
            .field("age", "number")
            .build();
        assert!(i.contains("export interface Person {"));
        assert!(i.contains("name: string;"));
        assert!(i.contains("age: number;"));
    }

    #[test]
    fn test_interface_with_optional_field() {
        let i = Interface::new("Config")
            .field("required", "string")
            .optional_field("optional", "number")
            .build();
        assert!(i.contains("required: string;"));
        assert!(i.contains("optional?: number;"));
    }

    #[test]
    fn test_private_interface() {
        let i = Interface::new("Internal")
            .private()
            .field("x", "number")
            .build();
        assert!(!i.contains("export"));
        assert!(i.contains("interface Internal {"));
    }

    #[test]
    fn test_readonly_field() {
        let i = Interface::new("Point")
            .field_with(InterfaceField::new("x", "number").readonly())
            .build();
        assert!(i.contains("readonly x: number;"));
    }
}
