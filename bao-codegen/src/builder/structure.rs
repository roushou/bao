//! Language-agnostic structure definitions.
//!
//! This module provides declarative specifications for structs, enums,
//! and their members that can be rendered to any target language.

use super::types::{TypeRef, Visibility};

/// A declarative specification for a struct or class.
///
/// Represents the *intent* of defining a data structure, independent
/// of any specific language syntax.
#[derive(Debug, Clone, PartialEq)]
pub struct StructSpec {
    /// Struct/class name.
    pub name: String,
    /// Documentation comment.
    pub doc: Option<String>,
    /// Fields in the struct.
    pub fields: Vec<FieldSpec>,
    /// Derive macros (Rust) or implemented interfaces (TS).
    pub derives: Vec<String>,
    /// Attributes (Rust) or decorators (TS/Python).
    pub attributes: Vec<AttributeSpec>,
    /// Visibility modifier.
    pub visibility: Visibility,
}

impl StructSpec {
    /// Create a new public struct spec.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            doc: None,
            fields: Vec::new(),
            derives: Vec::new(),
            attributes: Vec::new(),
            visibility: Visibility::Public,
        }
    }

    /// Set documentation comment.
    pub fn doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
    }

    /// Add a field.
    pub fn field(mut self, field: FieldSpec) -> Self {
        self.fields.push(field);
        self
    }

    /// Add multiple fields.
    pub fn fields(mut self, fields: impl IntoIterator<Item = FieldSpec>) -> Self {
        self.fields.extend(fields);
        self
    }

    /// Add a derive (Rust) or implement (TS interface).
    pub fn derive(mut self, name: impl Into<String>) -> Self {
        self.derives.push(name.into());
        self
    }

    /// Add multiple derives.
    pub fn derives(mut self, names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.derives.extend(names.into_iter().map(Into::into));
        self
    }

    /// Add an attribute.
    pub fn attribute(mut self, attr: AttributeSpec) -> Self {
        self.attributes.push(attr);
        self
    }

    /// Set visibility.
    pub fn visibility(mut self, vis: Visibility) -> Self {
        self.visibility = vis;
        self
    }

    /// Make this struct private.
    pub fn private(mut self) -> Self {
        self.visibility = Visibility::Private;
        self
    }

    /// Check if this struct has any fields.
    pub fn has_fields(&self) -> bool {
        !self.fields.is_empty()
    }
}

/// A field in a struct.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldSpec {
    /// Field name.
    pub name: String,
    /// Field type.
    pub ty: TypeRef,
    /// Documentation comment.
    pub doc: Option<String>,
    /// Whether the field is required (for interfaces/optional types).
    pub required: bool,
    /// Attributes on this field.
    pub attributes: Vec<AttributeSpec>,
    /// Visibility modifier.
    pub visibility: Visibility,
}

impl FieldSpec {
    /// Create a new required public field.
    pub fn new(name: impl Into<String>, ty: TypeRef) -> Self {
        Self {
            name: name.into(),
            ty,
            doc: None,
            required: true,
            attributes: Vec::new(),
            visibility: Visibility::Public,
        }
    }

    /// Set documentation comment.
    pub fn doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
    }

    /// Make this field optional.
    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    /// Add an attribute.
    pub fn attribute(mut self, attr: AttributeSpec) -> Self {
        self.attributes.push(attr);
        self
    }

    /// Set visibility.
    pub fn visibility(mut self, vis: Visibility) -> Self {
        self.visibility = vis;
        self
    }

    /// Make this field private.
    pub fn private(mut self) -> Self {
        self.visibility = Visibility::Private;
        self
    }
}

/// A declarative specification for an enum.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumSpec {
    /// Enum name.
    pub name: String,
    /// Documentation comment.
    pub doc: Option<String>,
    /// Variants in the enum.
    pub variants: Vec<VariantSpec>,
    /// Derive macros (Rust).
    pub derives: Vec<String>,
    /// Attributes on the enum.
    pub attributes: Vec<AttributeSpec>,
    /// Visibility modifier.
    pub visibility: Visibility,
}

impl EnumSpec {
    /// Create a new public enum spec.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            doc: None,
            variants: Vec::new(),
            derives: Vec::new(),
            attributes: Vec::new(),
            visibility: Visibility::Public,
        }
    }

    /// Set documentation comment.
    pub fn doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
    }

    /// Add a variant.
    pub fn variant(mut self, variant: VariantSpec) -> Self {
        self.variants.push(variant);
        self
    }

    /// Add multiple variants.
    pub fn variants(mut self, variants: impl IntoIterator<Item = VariantSpec>) -> Self {
        self.variants.extend(variants);
        self
    }

    /// Add a simple unit variant by name.
    pub fn unit_variant(mut self, name: impl Into<String>) -> Self {
        self.variants.push(VariantSpec::unit(name));
        self
    }

    /// Add a derive.
    pub fn derive(mut self, name: impl Into<String>) -> Self {
        self.derives.push(name.into());
        self
    }

    /// Add multiple derives.
    pub fn derives(mut self, names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.derives.extend(names.into_iter().map(Into::into));
        self
    }

    /// Add an attribute.
    pub fn attribute(mut self, attr: AttributeSpec) -> Self {
        self.attributes.push(attr);
        self
    }

    /// Set visibility.
    pub fn visibility(mut self, vis: Visibility) -> Self {
        self.visibility = vis;
        self
    }

    /// Make this enum private.
    pub fn private(mut self) -> Self {
        self.visibility = Visibility::Private;
        self
    }
}

/// A variant in an enum.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantSpec {
    /// Variant name.
    pub name: String,
    /// Documentation comment.
    pub doc: Option<String>,
    /// Variant kind (unit, tuple, struct).
    pub kind: VariantKind,
    /// Attributes on this variant.
    pub attributes: Vec<AttributeSpec>,
}

impl VariantSpec {
    /// Create a unit variant.
    pub fn unit(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            doc: None,
            kind: VariantKind::Unit,
            attributes: Vec::new(),
        }
    }

    /// Create a tuple variant.
    pub fn tuple(name: impl Into<String>, fields: Vec<TypeRef>) -> Self {
        Self {
            name: name.into(),
            doc: None,
            kind: VariantKind::Tuple(fields),
            attributes: Vec::new(),
        }
    }

    /// Create a struct variant.
    pub fn struct_(name: impl Into<String>, fields: Vec<FieldSpec>) -> Self {
        Self {
            name: name.into(),
            doc: None,
            kind: VariantKind::Struct(fields),
            attributes: Vec::new(),
        }
    }

    /// Set documentation comment.
    pub fn doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
    }

    /// Add an attribute.
    pub fn attribute(mut self, attr: AttributeSpec) -> Self {
        self.attributes.push(attr);
        self
    }
}

/// Kind of enum variant.
#[derive(Debug, Clone, PartialEq)]
pub enum VariantKind {
    /// Unit variant: `None`, `Empty`.
    Unit,
    /// Tuple variant: `Some(T)`, `Pair(A, B)`.
    Tuple(Vec<TypeRef>),
    /// Struct variant: `Point { x: i32, y: i32 }`.
    Struct(Vec<FieldSpec>),
}

/// An attribute or decorator on a type or member.
#[derive(Debug, Clone, PartialEq)]
pub struct AttributeSpec {
    /// Attribute path (e.g., "serde::rename", "clap::arg").
    pub path: String,
    /// Attribute arguments.
    pub args: Vec<AttributeArg>,
}

impl AttributeSpec {
    /// Create a simple attribute with no arguments.
    pub fn simple(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            args: Vec::new(),
        }
    }

    /// Create an attribute with a single unnamed argument.
    pub fn with_value(path: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            args: vec![AttributeArg::Positional(value.into())],
        }
    }

    /// Add a positional argument.
    pub fn arg(mut self, value: impl Into<String>) -> Self {
        self.args.push(AttributeArg::Positional(value.into()));
        self
    }

    /// Add a named argument.
    pub fn named(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.args
            .push(AttributeArg::Named(name.into(), value.into()));
        self
    }

    /// Add a flag argument (name with no value).
    pub fn flag(mut self, name: impl Into<String>) -> Self {
        self.args.push(AttributeArg::Flag(name.into()));
        self
    }
}

/// An argument to an attribute.
#[derive(Debug, Clone, PartialEq)]
pub enum AttributeArg {
    /// Positional argument.
    Positional(String),
    /// Named argument: `key = value`.
    Named(String, String),
    /// Flag (name only, no value).
    Flag(String),
}

/// Trait for rendering structure specs to language-specific code.
///
/// Implement this trait to support rendering structs and enums
/// in a new target language.
pub trait StructureRenderer {
    /// Render a struct specification to code.
    fn render_struct(&self, spec: &StructSpec) -> String;

    /// Render an enum specification to code.
    fn render_enum(&self, spec: &EnumSpec) -> String;

    /// Render a field specification to code.
    fn render_field(&self, spec: &FieldSpec) -> String;

    /// Render a variant specification to code.
    fn render_variant(&self, spec: &VariantSpec) -> String;

    /// Render an attribute specification to code.
    fn render_attribute(&self, spec: &AttributeSpec) -> String;

    /// Render a visibility modifier to code.
    fn render_visibility(&self, vis: Visibility) -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_struct_spec() {
        let spec = StructSpec::new("User")
            .doc("A user in the system")
            .derive("Debug")
            .derive("Clone")
            .field(FieldSpec::new("id", TypeRef::int()))
            .field(FieldSpec::new("name", TypeRef::string()));

        assert_eq!(spec.name, "User");
        assert_eq!(spec.doc, Some("A user in the system".into()));
        assert_eq!(spec.derives, vec!["Debug", "Clone"]);
        assert_eq!(spec.fields.len(), 2);
        assert!(spec.has_fields());
    }

    #[test]
    fn test_field_spec() {
        let field = FieldSpec::new("email", TypeRef::string())
            .doc("User's email address")
            .optional()
            .private();

        assert_eq!(field.name, "email");
        assert!(!field.required);
        assert!(field.visibility.is_private());
    }

    #[test]
    fn test_enum_spec() {
        let spec = EnumSpec::new("Status")
            .derive("Debug")
            .unit_variant("Pending")
            .unit_variant("Active")
            .variant(VariantSpec::tuple("Error", vec![TypeRef::string()]));

        assert_eq!(spec.name, "Status");
        assert_eq!(spec.variants.len(), 3);
    }

    #[test]
    fn test_variant_kinds() {
        let unit = VariantSpec::unit("None");
        assert!(matches!(unit.kind, VariantKind::Unit));

        let tuple = VariantSpec::tuple("Some", vec![TypeRef::int()]);
        assert!(matches!(tuple.kind, VariantKind::Tuple(ref fields) if fields.len() == 1));

        let struct_ = VariantSpec::struct_(
            "Point",
            vec![
                FieldSpec::new("x", TypeRef::int()),
                FieldSpec::new("y", TypeRef::int()),
            ],
        );
        assert!(matches!(struct_.kind, VariantKind::Struct(ref fields) if fields.len() == 2));
    }

    #[test]
    fn test_attribute_spec() {
        let simple = AttributeSpec::simple("derive");
        assert!(simple.args.is_empty());

        let with_value = AttributeSpec::with_value("serde", "rename_all = \"camelCase\"");
        assert_eq!(with_value.args.len(), 1);

        let complex = AttributeSpec::simple("clap")
            .arg("long")
            .named("short", "'v'")
            .flag("global");
        assert_eq!(complex.args.len(), 3);
    }
}
