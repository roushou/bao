//! TypeScript type mapper implementation.

#[cfg(test)]
use baobao_codegen::builder::TypeRef;
use baobao_codegen::{
    builder::{PrimitiveType, TypeMapper as CodeIRTypeMapper},
    language::TypeMapper,
};
use baobao_core::{ArgType, ContextFieldType, DatabaseType};

/// TypeScript type mapper implementation.
pub struct TypeScriptTypeMapper;

/// TypeScript Code IR type mapper implementation.
///
/// Maps language-agnostic TypeRef types to TypeScript type syntax.
#[derive(Debug, Clone, Copy, Default)]
pub struct TypeScriptCodeTypeMapper;

impl CodeIRTypeMapper for TypeScriptCodeTypeMapper {
    fn map_primitive(&self, ty: PrimitiveType) -> String {
        match ty {
            PrimitiveType::String => "string".to_string(),
            PrimitiveType::Int | PrimitiveType::UInt | PrimitiveType::Float => "number".to_string(),
            PrimitiveType::Bool => "boolean".to_string(),
            PrimitiveType::Path => "string".to_string(),
            PrimitiveType::Duration => "number".to_string(), // milliseconds
            PrimitiveType::Char => "string".to_string(),
            PrimitiveType::Byte => "number".to_string(),
        }
    }

    fn map_optional(&self, inner: &str) -> String {
        format!("{} | undefined", inner)
    }

    fn map_array(&self, inner: &str) -> String {
        format!("{}[]", inner)
    }

    fn map_result(&self, ok: &str, _err: &str) -> String {
        // TypeScript doesn't have a built-in Result type
        // Just return the success type (errors are handled differently)
        ok.to_string()
    }

    fn map_unit(&self) -> String {
        "void".to_string()
    }
}

impl TypeMapper for TypeScriptTypeMapper {
    fn language(&self) -> &'static str {
        "typescript"
    }

    fn map_arg_type(&self, arg_type: ArgType) -> &'static str {
        match arg_type {
            ArgType::String => "string",
            ArgType::Int => "number",
            ArgType::Float => "number",
            ArgType::Bool => "boolean",
            ArgType::Path => "string",
        }
    }

    fn map_optional_arg_type(&self, arg_type: ArgType) -> String {
        format!("{} | undefined", self.map_arg_type(arg_type))
    }

    fn map_context_type(&self, field_type: &ContextFieldType) -> &'static str {
        match field_type {
            // Bun's native SQLite
            ContextFieldType::Database(DatabaseType::Sqlite) => "Database",
            // For Postgres/MySQL, we'll use placeholder types for now
            ContextFieldType::Database(DatabaseType::Postgres) => "unknown",
            ContextFieldType::Database(DatabaseType::Mysql) => "unknown",
            ContextFieldType::Http => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typescript_arg_types() {
        let mapper = TypeScriptTypeMapper;

        assert_eq!(mapper.map_arg_type(ArgType::String), "string");
        assert_eq!(mapper.map_arg_type(ArgType::Int), "number");
        assert_eq!(mapper.map_arg_type(ArgType::Float), "number");
        assert_eq!(mapper.map_arg_type(ArgType::Bool), "boolean");
        assert_eq!(mapper.map_arg_type(ArgType::Path), "string");
    }

    #[test]
    fn test_typescript_optional_types() {
        let mapper = TypeScriptTypeMapper;

        assert_eq!(
            mapper.map_optional_arg_type(ArgType::String),
            "string | undefined"
        );
        assert_eq!(
            mapper.map_optional_arg_type(ArgType::Int),
            "number | undefined"
        );
    }

    #[test]
    fn test_typescript_context_types() {
        let mapper = TypeScriptTypeMapper;

        assert_eq!(
            mapper.map_context_type(&ContextFieldType::Database(DatabaseType::Sqlite)),
            "Database"
        );
    }

    #[test]
    fn test_typescript_code_type_mapper_primitives() {
        let mapper = TypeScriptCodeTypeMapper;

        assert_eq!(mapper.map_primitive(PrimitiveType::String), "string");
        assert_eq!(mapper.map_primitive(PrimitiveType::Int), "number");
        assert_eq!(mapper.map_primitive(PrimitiveType::UInt), "number");
        assert_eq!(mapper.map_primitive(PrimitiveType::Float), "number");
        assert_eq!(mapper.map_primitive(PrimitiveType::Bool), "boolean");
        assert_eq!(mapper.map_primitive(PrimitiveType::Path), "string");
        assert_eq!(mapper.map_primitive(PrimitiveType::Duration), "number");
    }

    #[test]
    fn test_typescript_code_type_mapper_complex() {
        let mapper = TypeScriptCodeTypeMapper;

        assert_eq!(mapper.map_optional("string"), "string | undefined");
        assert_eq!(mapper.map_array("number"), "number[]");
        assert_eq!(mapper.map_result("void", "Error"), "void");
        assert_eq!(mapper.map_unit(), "void");
    }

    #[test]
    fn test_typescript_code_type_mapper_render() {
        let mapper = TypeScriptCodeTypeMapper;

        // Test rendering complex TypeRef
        let string_type = TypeRef::string();
        assert_eq!(mapper.render_type(&string_type), "string");

        let opt_string = TypeRef::optional(TypeRef::string());
        assert_eq!(mapper.render_type(&opt_string), "string | undefined");

        let arr_int = TypeRef::array(TypeRef::int());
        assert_eq!(mapper.render_type(&arr_int), "number[]");

        let unit = TypeRef::unit();
        assert_eq!(mapper.render_type(&unit), "void");
    }
}
