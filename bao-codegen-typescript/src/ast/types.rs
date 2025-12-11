//! TypeScript type alias and union builders.

use baobao_codegen::{CodeBuilder, CodeFragment, Renderable};

/// A field in a TypeScript object type.
#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: String,
    pub doc: Option<String>,
    pub optional: bool,
    pub readonly: bool,
}

impl Field {
    pub fn new(name: impl Into<String>, ty: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ty: ty.into(),
            doc: None,
            optional: false,
            readonly: false,
        }
    }

    pub fn doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
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

/// Builder for TypeScript object types (`type Foo = { ... }`).
#[derive(Debug, Clone)]
pub struct ObjectType {
    name: String,
    doc: Option<String>,
    fields: Vec<Field>,
    exported: bool,
}

impl ObjectType {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            doc: None,
            fields: Vec::new(),
            exported: true,
        }
    }

    pub fn doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
    }

    pub fn field(mut self, field: Field) -> Self {
        self.fields.push(field);
        self
    }

    pub fn private(mut self) -> Self {
        self.exported = false;
        self
    }

    /// Render the object type to a CodeBuilder.
    pub fn render(&self, builder: CodeBuilder) -> CodeBuilder {
        let export = if self.exported { "export " } else { "" };

        let builder = if let Some(doc) = &self.doc {
            builder.jsdoc(doc)
        } else {
            builder
        };

        if self.fields.is_empty() {
            builder.line(&format!("{}type {} = {{}};", export, self.name))
        } else {
            let builder = builder
                .line(&format!("{}type {} = {{", export, self.name))
                .indent();
            self.render_fields(builder).dedent().line("};")
        }
    }

    fn render_fields(&self, builder: CodeBuilder) -> CodeBuilder {
        self.fields.iter().fold(builder, |b, field| {
            let b = if let Some(doc) = &field.doc {
                b.jsdoc(doc)
            } else {
                b
            };

            let readonly = if field.readonly { "readonly " } else { "" };
            let optional = if field.optional { "?" } else { "" };

            b.line(&format!(
                "{}{}{}: {};",
                readonly, field.name, optional, field.ty
            ))
        })
    }

    /// Build the object type as a string.
    pub fn build(&self) -> String {
        self.render(CodeBuilder::typescript()).build()
    }

    /// Convert fields to code fragments.
    fn fields_to_fragments(&self) -> Vec<CodeFragment> {
        self.fields
            .iter()
            .flat_map(|field| {
                let mut fragments = Vec::new();
                if let Some(doc) = &field.doc {
                    fragments.push(CodeFragment::JsDoc(doc.clone()));
                }
                let readonly = if field.readonly { "readonly " } else { "" };
                let optional = if field.optional { "?" } else { "" };
                fragments.push(CodeFragment::Line(format!(
                    "{}{}{}: {};",
                    readonly, field.name, optional, field.ty
                )));
                fragments
            })
            .collect()
    }
}

impl Renderable for ObjectType {
    fn to_fragments(&self) -> Vec<CodeFragment> {
        let export = if self.exported { "export " } else { "" };
        let mut fragments = Vec::new();

        if let Some(doc) = &self.doc {
            fragments.push(CodeFragment::JsDoc(doc.clone()));
        }

        if self.fields.is_empty() {
            fragments.push(CodeFragment::Line(format!(
                "{}type {} = {{}};",
                export, self.name
            )));
        } else {
            fragments.push(CodeFragment::Block {
                header: format!("{}type {} = {{", export, self.name),
                body: self.fields_to_fragments(),
                close: Some("};".to_string()),
            });
        }

        fragments
    }
}

/// Builder for TypeScript type aliases.
#[derive(Debug, Clone)]
pub struct TypeAlias {
    name: String,
    doc: Option<String>,
    ty: String,
    exported: bool,
}

impl TypeAlias {
    pub fn new(name: impl Into<String>, ty: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            doc: None,
            ty: ty.into(),
            exported: true,
        }
    }

    pub fn doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
    }

    pub fn private(mut self) -> Self {
        self.exported = false;
        self
    }

    /// Render the type alias to a CodeBuilder.
    pub fn render(&self, builder: CodeBuilder) -> CodeBuilder {
        let export = if self.exported { "export " } else { "" };

        let builder = if let Some(doc) = &self.doc {
            builder.jsdoc(doc)
        } else {
            builder
        };

        builder.line(&format!("{}type {} = {};", export, self.name, self.ty))
    }

    /// Build the type alias as a string.
    pub fn build(&self) -> String {
        self.render(CodeBuilder::typescript()).build()
    }
}

impl Renderable for TypeAlias {
    fn to_fragments(&self) -> Vec<CodeFragment> {
        let export = if self.exported { "export " } else { "" };
        let mut fragments = Vec::new();

        if let Some(doc) = &self.doc {
            fragments.push(CodeFragment::JsDoc(doc.clone()));
        }

        fragments.push(CodeFragment::Line(format!(
            "{}type {} = {};",
            export, self.name, self.ty
        )));

        fragments
    }
}

/// Builder for TypeScript union types.
#[derive(Debug, Clone)]
pub struct Union {
    name: String,
    doc: Option<String>,
    variants: Vec<String>,
    exported: bool,
}

impl Union {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            doc: None,
            variants: Vec::new(),
            exported: true,
        }
    }

    pub fn doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
    }

    pub fn variant(mut self, variant: impl Into<String>) -> Self {
        self.variants.push(variant.into());
        self
    }

    pub fn private(mut self) -> Self {
        self.exported = false;
        self
    }

    /// Render the union type to a CodeBuilder.
    pub fn render(&self, builder: CodeBuilder) -> CodeBuilder {
        let export = if self.exported { "export " } else { "" };

        let builder = if let Some(doc) = &self.doc {
            builder.jsdoc(doc)
        } else {
            builder
        };

        let variants_str = self.variants.join(" | ");
        builder.line(&format!("{}type {} = {};", export, self.name, variants_str))
    }

    /// Build the union type as a string.
    pub fn build(&self) -> String {
        self.render(CodeBuilder::typescript()).build()
    }
}

impl Renderable for Union {
    fn to_fragments(&self) -> Vec<CodeFragment> {
        let export = if self.exported { "export " } else { "" };
        let mut fragments = Vec::new();

        if let Some(doc) = &self.doc {
            fragments.push(CodeFragment::JsDoc(doc.clone()));
        }

        let variants_str = self.variants.join(" | ");
        fragments.push(CodeFragment::Line(format!(
            "{}type {} = {};",
            export, self.name, variants_str
        )));

        fragments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_type_empty() {
        let t = ObjectType::new("Empty").build();
        assert_eq!(t, "export type Empty = {};\n");
    }

    #[test]
    fn test_object_type_with_fields() {
        let t = ObjectType::new("Person")
            .field(Field::new("name", "string"))
            .field(Field::new("age", "number"))
            .build();
        assert!(t.contains("export type Person = {"));
        assert!(t.contains("name: string;"));
        assert!(t.contains("age: number;"));
        assert!(t.contains("};"));
    }

    #[test]
    fn test_object_type_with_optional_field() {
        let t = ObjectType::new("Config")
            .field(Field::new("debug", "boolean").optional())
            .build();
        assert!(t.contains("debug?: boolean;"));
    }

    #[test]
    fn test_object_type_with_readonly_field() {
        let t = ObjectType::new("Point")
            .field(Field::new("x", "number").readonly())
            .build();
        assert!(t.contains("readonly x: number;"));
    }

    #[test]
    fn test_type_alias() {
        let t = TypeAlias::new("UserId", "string").build();
        assert_eq!(t, "export type UserId = string;\n");
    }

    #[test]
    fn test_type_alias_with_doc() {
        let t = TypeAlias::new("Callback", "() => void")
            .doc("A callback function")
            .build();
        assert!(t.contains("/** A callback function */"));
        assert!(t.contains("export type Callback = () => void;"));
    }

    #[test]
    fn test_private_type_alias() {
        let t = TypeAlias::new("Internal", "number").private().build();
        assert!(!t.contains("export"));
        assert!(t.contains("type Internal = number;"));
    }

    #[test]
    fn test_union() {
        let u = Union::new("Status")
            .variant("\"pending\"")
            .variant("\"active\"")
            .variant("\"completed\"")
            .build();
        assert!(u.contains("export type Status = \"pending\" | \"active\" | \"completed\";"));
    }

    #[test]
    fn test_union_with_doc() {
        let u = Union::new("Result")
            .doc("Success or failure")
            .variant("Success")
            .variant("Failure")
            .build();
        assert!(u.contains("/** Success or failure */"));
        assert!(u.contains("export type Result = Success | Failure;"));
    }
}
