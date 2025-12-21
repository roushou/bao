//! TypeScript implementation of StructureRenderer for Code IR types.
//!
//! This module provides rendering of language-agnostic structure specifications
//! (`StructSpec`, `EnumSpec`) to TypeScript code.
//!
//! # TypeScript Mapping
//!
//! - `StructSpec` → TypeScript `interface` or `type`
//! - `EnumSpec` → TypeScript `type` union or const enum
//! - `Visibility` → `export` modifier (Public) or nothing (Private)

use baobao_codegen::builder::{
    AttributeSpec, EnumSpec, FieldSpec, StructSpec, StructureRenderer, TypeMapper, VariantKind,
    VariantSpec, Visibility,
};

use crate::type_mapper::TypeScriptCodeTypeMapper;

/// TypeScript implementation of StructureRenderer.
///
/// Renders language-agnostic `StructSpec` and `EnumSpec` to TypeScript code.
#[derive(Debug, Clone, Copy, Default)]
pub struct TypeScriptStructureRenderer {
    type_mapper: TypeScriptCodeTypeMapper,
}

impl TypeScriptStructureRenderer {
    /// Create a new TypeScript structure renderer.
    pub fn new() -> Self {
        Self {
            type_mapper: TypeScriptCodeTypeMapper,
        }
    }

    /// Render doc comment if present.
    fn render_doc(&self, doc: &Option<String>, indent: &str) -> String {
        match doc {
            Some(d) => format!("{}/** {} */\n", indent, d),
            None => String::new(),
        }
    }
}

impl StructureRenderer for TypeScriptStructureRenderer {
    /// Render a struct as a TypeScript interface.
    fn render_struct(&self, spec: &StructSpec) -> String {
        let mut result = String::new();

        // Doc comment
        result.push_str(&self.render_doc(&spec.doc, ""));

        // Visibility and interface declaration
        let vis = self.render_visibility(spec.visibility);
        if !vis.is_empty() {
            result.push_str(vis);
            result.push(' ');
        }
        result.push_str("interface ");
        result.push_str(&spec.name);

        if spec.fields.is_empty() {
            result.push_str(" {}\n");
        } else {
            result.push_str(" {\n");
            for field in &spec.fields {
                result.push_str(&self.render_field(field));
            }
            result.push_str("}\n");
        }

        result
    }

    /// Render an enum as a TypeScript type union.
    fn render_enum(&self, spec: &EnumSpec) -> String {
        let mut result = String::new();

        // Doc comment
        result.push_str(&self.render_doc(&spec.doc, ""));

        // Visibility and type declaration
        let vis = self.render_visibility(spec.visibility);
        if !vis.is_empty() {
            result.push_str(vis);
            result.push(' ');
        }
        result.push_str("type ");
        result.push_str(&spec.name);
        result.push_str(" =\n");

        // Render variants as a discriminated union
        let variant_count = spec.variants.len();
        for (i, variant) in spec.variants.iter().enumerate() {
            result.push_str(&self.render_variant(variant));
            if i < variant_count - 1 {
                // Remove trailing semicolon and newline, add pipe
                if result.ends_with(";\n") {
                    result.truncate(result.len() - 2);
                }
                result.push_str(" |\n");
            }
        }

        result
    }

    /// Render a field as a TypeScript interface property.
    fn render_field(&self, spec: &FieldSpec) -> String {
        let mut result = String::new();

        // Doc comment
        result.push_str(&self.render_doc(&spec.doc, "  "));

        // Field declaration (no visibility in interface fields)
        result.push_str("  ");
        result.push_str(&spec.name);

        // Check if optional (TypeRef::Optional)
        if spec.ty.is_optional() {
            result.push('?');
        }

        result.push_str(": ");
        // For optional types, render the inner type
        let type_str = if spec.ty.is_optional() {
            if let Some(inner) = spec.ty.inner_type() {
                self.type_mapper.render_type(inner)
            } else {
                self.type_mapper.render_type(&spec.ty)
            }
        } else {
            self.type_mapper.render_type(&spec.ty)
        };
        result.push_str(&type_str);
        result.push_str(";\n");

        result
    }

    /// Render a variant as part of a TypeScript discriminated union.
    fn render_variant(&self, spec: &VariantSpec) -> String {
        let mut result = String::new();

        // Doc comment
        result.push_str(&self.render_doc(&spec.doc, "  "));

        result.push_str("  ");

        match &spec.kind {
            VariantKind::Unit => {
                // Unit variant: { kind: "VariantName" }
                result.push_str("{ kind: \"");
                result.push_str(&spec.name);
                result.push_str("\" };\n");
            }
            VariantKind::Tuple(fields) => {
                // Tuple variant: { kind: "VariantName", value: T } or { kind: "...", values: [T, U] }
                result.push_str("{ kind: \"");
                result.push_str(&spec.name);
                result.push('"');

                if fields.len() == 1 {
                    result.push_str("; value: ");
                    result.push_str(&self.type_mapper.render_type(&fields[0]));
                } else {
                    result.push_str("; values: [");
                    let types: Vec<String> = fields
                        .iter()
                        .map(|f| self.type_mapper.render_type(f))
                        .collect();
                    result.push_str(&types.join(", "));
                    result.push(']');
                }
                result.push_str(" };\n");
            }
            VariantKind::Struct(fields) => {
                // Struct variant: { kind: "VariantName", field1: T, field2: U }
                result.push_str("{ kind: \"");
                result.push_str(&spec.name);
                result.push('"');

                for field in fields {
                    result.push_str("; ");
                    result.push_str(&field.name);
                    result.push_str(": ");
                    result.push_str(&self.type_mapper.render_type(&field.ty));
                }
                result.push_str(" };\n");
            }
        }

        result
    }

    /// Render an attribute (TypeScript uses decorators, but we'll skip for now).
    fn render_attribute(&self, _spec: &AttributeSpec) -> String {
        // TypeScript doesn't have attributes in the same way as Rust
        // Decorators are different and typically used with classes
        String::new()
    }

    fn render_visibility(&self, vis: Visibility) -> &'static str {
        match vis {
            Visibility::Public => "export",
            Visibility::Private => "",
            // TypeScript doesn't have crate/super visibility
            Visibility::Crate => "",
            Visibility::Super => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use baobao_codegen::builder::TypeRef;

    use super::*;

    #[test]
    fn test_render_simple_interface() {
        let renderer = TypeScriptStructureRenderer::new();
        let spec = StructSpec::new("User")
            .doc("A user in the system")
            .field(FieldSpec::new("id", TypeRef::int()))
            .field(FieldSpec::new("name", TypeRef::string()));

        let result = renderer.render_struct(&spec);
        assert!(result.contains("/** A user in the system */"));
        assert!(result.contains("export interface User {"));
        assert!(result.contains("id: number;"));
        assert!(result.contains("name: string;"));
    }

    #[test]
    fn test_render_interface_with_optional() {
        let renderer = TypeScriptStructureRenderer::new();
        let spec = StructSpec::new("Config")
            .field(FieldSpec::new("name", TypeRef::string()))
            .field(FieldSpec::new("timeout", TypeRef::optional(TypeRef::int())));

        let result = renderer.render_struct(&spec);
        assert!(result.contains("name: string;"));
        assert!(result.contains("timeout?: number;"));
    }

    #[test]
    fn test_render_type_union() {
        let renderer = TypeScriptStructureRenderer::new();
        let spec = EnumSpec::new("Status")
            .doc("Status of an operation")
            .unit_variant("Pending")
            .unit_variant("Active")
            .variant(VariantSpec::tuple("Error", vec![TypeRef::string()]));

        let result = renderer.render_enum(&spec);
        assert!(result.contains("/** Status of an operation */"));
        assert!(result.contains("export type Status ="));
        assert!(result.contains("{ kind: \"Pending\" }"));
        assert!(result.contains("{ kind: \"Active\" }"));
        assert!(result.contains("{ kind: \"Error\"; value: string }"));
    }

    #[test]
    fn test_render_private_interface() {
        let renderer = TypeScriptStructureRenderer::new();
        let spec = StructSpec::new("Internal").private();

        let result = renderer.render_struct(&spec);
        assert!(result.contains("interface Internal {}"));
        assert!(!result.contains("export"));
    }

    #[test]
    fn test_render_struct_variant() {
        let renderer = TypeScriptStructureRenderer::new();
        let spec = EnumSpec::new("Event").variant(VariantSpec::struct_(
            "Click",
            vec![
                FieldSpec::new("x", TypeRef::int()),
                FieldSpec::new("y", TypeRef::int()),
            ],
        ));

        let result = renderer.render_enum(&spec);
        assert!(result.contains("{ kind: \"Click\"; x: number; y: number }"));
    }

    #[test]
    fn test_render_empty_interface() {
        let renderer = TypeScriptStructureRenderer::new();
        let spec = StructSpec::new("Empty");

        let result = renderer.render_struct(&spec);
        assert!(result.contains("export interface Empty {}"));
    }
}
