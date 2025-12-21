//! Rust implementation of StructureRenderer for Code IR types.
//!
//! This module provides rendering of language-agnostic structure specifications
//! (`StructSpec`, `EnumSpec`) to Rust code.

use baobao_codegen::builder::{
    AttributeArg, AttributeSpec, EnumSpec, FieldSpec, StructSpec, StructureRenderer, TypeMapper,
    VariantKind, VariantSpec, Visibility,
};

use crate::type_mapper::RustCodeTypeMapper;

/// Rust implementation of StructureRenderer.
///
/// Renders language-agnostic `StructSpec` and `EnumSpec` to Rust code.
#[derive(Debug, Clone, Copy, Default)]
pub struct RustStructureRenderer {
    type_mapper: RustCodeTypeMapper,
}

impl RustStructureRenderer {
    /// Create a new Rust structure renderer.
    pub fn new() -> Self {
        Self {
            type_mapper: RustCodeTypeMapper,
        }
    }

    /// Render doc comment if present.
    fn render_doc(&self, doc: &Option<String>, indent: &str) -> String {
        match doc {
            Some(d) => format!("{}/// {}\n", indent, d),
            None => String::new(),
        }
    }

    /// Render derives if present.
    fn render_derives(&self, derives: &[String]) -> String {
        if derives.is_empty() {
            String::new()
        } else {
            format!("#[derive({})]\n", derives.join(", "))
        }
    }
}

impl StructureRenderer for RustStructureRenderer {
    fn render_struct(&self, spec: &StructSpec) -> String {
        let mut result = String::new();

        // Doc comment
        result.push_str(&self.render_doc(&spec.doc, ""));

        // Attributes
        for attr in &spec.attributes {
            result.push_str(&self.render_attribute(attr));
            result.push('\n');
        }

        // Derives
        result.push_str(&self.render_derives(&spec.derives));

        // Visibility and struct declaration
        let vis = self.render_visibility(spec.visibility);
        if !vis.is_empty() {
            result.push_str(vis);
            result.push(' ');
        }
        result.push_str("struct ");
        result.push_str(&spec.name);

        if spec.fields.is_empty() {
            result.push_str(";\n");
        } else {
            result.push_str(" {\n");
            for field in &spec.fields {
                result.push_str(&self.render_field(field));
            }
            result.push_str("}\n");
        }

        result
    }

    fn render_enum(&self, spec: &EnumSpec) -> String {
        let mut result = String::new();

        // Doc comment
        result.push_str(&self.render_doc(&spec.doc, ""));

        // Attributes
        for attr in &spec.attributes {
            result.push_str(&self.render_attribute(attr));
            result.push('\n');
        }

        // Derives
        result.push_str(&self.render_derives(&spec.derives));

        // Visibility and enum declaration
        let vis = self.render_visibility(spec.visibility);
        if !vis.is_empty() {
            result.push_str(vis);
            result.push(' ');
        }
        result.push_str("enum ");
        result.push_str(&spec.name);
        result.push_str(" {\n");

        for variant in &spec.variants {
            result.push_str(&self.render_variant(variant));
        }

        result.push_str("}\n");

        result
    }

    fn render_field(&self, spec: &FieldSpec) -> String {
        let mut result = String::new();

        // Doc comment
        result.push_str(&self.render_doc(&spec.doc, "    "));

        // Attributes
        for attr in &spec.attributes {
            result.push_str("    ");
            result.push_str(&self.render_attribute(attr));
            result.push('\n');
        }

        // Field declaration
        result.push_str("    ");
        let vis = self.render_visibility(spec.visibility);
        if !vis.is_empty() {
            result.push_str(vis);
            result.push(' ');
        }
        result.push_str(&spec.name);
        result.push_str(": ");
        result.push_str(&self.type_mapper.render_type(&spec.ty));
        result.push_str(",\n");

        result
    }

    fn render_variant(&self, spec: &VariantSpec) -> String {
        let mut result = String::new();

        // Doc comment
        result.push_str(&self.render_doc(&spec.doc, "    "));

        // Attributes
        for attr in &spec.attributes {
            result.push_str("    ");
            result.push_str(&self.render_attribute(attr));
            result.push('\n');
        }

        // Variant declaration
        result.push_str("    ");
        result.push_str(&spec.name);

        match &spec.kind {
            VariantKind::Unit => {}
            VariantKind::Tuple(fields) => {
                let field_types: Vec<String> = fields
                    .iter()
                    .map(|f| self.type_mapper.render_type(f))
                    .collect();
                result.push('(');
                result.push_str(&field_types.join(", "));
                result.push(')');
            }
            VariantKind::Struct(fields) => {
                result.push_str(" {\n");
                for field in fields {
                    result.push_str("        ");
                    result.push_str(&field.name);
                    result.push_str(": ");
                    result.push_str(&self.type_mapper.render_type(&field.ty));
                    result.push_str(",\n");
                }
                result.push_str("    }");
            }
        }

        result.push_str(",\n");

        result
    }

    fn render_attribute(&self, spec: &AttributeSpec) -> String {
        let mut result = String::from("#[");
        result.push_str(&spec.path);

        if !spec.args.is_empty() {
            result.push('(');
            let args: Vec<String> = spec
                .args
                .iter()
                .map(|arg| match arg {
                    AttributeArg::Positional(value) => value.clone(),
                    AttributeArg::Named(name, value) => format!("{} = {}", name, value),
                    AttributeArg::Flag(name) => name.clone(),
                })
                .collect();
            result.push_str(&args.join(", "));
            result.push(')');
        }

        result.push(']');
        result
    }

    fn render_visibility(&self, vis: Visibility) -> &'static str {
        match vis {
            Visibility::Public => "pub",
            Visibility::Private => "",
            Visibility::Crate => "pub(crate)",
            Visibility::Super => "pub(super)",
        }
    }
}

#[cfg(test)]
mod tests {
    use baobao_codegen::builder::TypeRef;

    use super::*;

    #[test]
    fn test_render_simple_struct() {
        let renderer = RustStructureRenderer::new();
        let spec = StructSpec::new("User")
            .doc("A user in the system")
            .derive("Debug")
            .derive("Clone")
            .field(FieldSpec::new("id", TypeRef::int()))
            .field(FieldSpec::new("name", TypeRef::string()));

        let result = renderer.render_struct(&spec);
        assert!(result.contains("/// A user in the system"));
        assert!(result.contains("#[derive(Debug, Clone)]"));
        assert!(result.contains("pub struct User {"));
        assert!(result.contains("pub id: i64,"));
        assert!(result.contains("pub name: String,"));
    }

    #[test]
    fn test_render_enum() {
        let renderer = RustStructureRenderer::new();
        let spec = EnumSpec::new("Status")
            .derive("Debug")
            .derive("Clone")
            .unit_variant("Pending")
            .unit_variant("Active")
            .variant(VariantSpec::tuple("Error", vec![TypeRef::string()]));

        let result = renderer.render_enum(&spec);
        assert!(result.contains("#[derive(Debug, Clone)]"));
        assert!(result.contains("pub enum Status {"));
        assert!(result.contains("Pending,"));
        assert!(result.contains("Active,"));
        assert!(result.contains("Error(String),"));
    }

    #[test]
    fn test_render_struct_with_attributes() {
        let renderer = RustStructureRenderer::new();
        let spec = StructSpec::new("Args")
            .derive("Debug")
            .attribute(AttributeSpec::simple("derive").arg("Args"))
            .field(
                FieldSpec::new("name", TypeRef::string()).attribute(
                    AttributeSpec::simple("clap")
                        .arg("long")
                        .named("short", "'n'"),
                ),
            );

        let result = renderer.render_struct(&spec);
        assert!(result.contains("#[derive(Args)]"));
        assert!(result.contains("#[clap(long, short = 'n')]"));
    }

    #[test]
    fn test_render_private_struct() {
        let renderer = RustStructureRenderer::new();
        let spec = StructSpec::new("Internal").private();

        let result = renderer.render_struct(&spec);
        assert!(result.contains("struct Internal;"));
        assert!(!result.contains("pub struct"));
    }

    #[test]
    fn test_render_struct_variant() {
        let renderer = RustStructureRenderer::new();
        let spec = EnumSpec::new("Event")
            .derive("Debug")
            .variant(VariantSpec::struct_(
                "Click",
                vec![
                    FieldSpec::new("x", TypeRef::int()),
                    FieldSpec::new("y", TypeRef::int()),
                ],
            ));

        let result = renderer.render_enum(&spec);
        assert!(result.contains("Click {"));
        assert!(result.contains("x: i64,"));
        assert!(result.contains("y: i64,"));
    }
}
