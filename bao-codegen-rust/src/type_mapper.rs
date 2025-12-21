//! Rust type mapper implementation.

#[cfg(test)]
use baobao_codegen::builder::TypeRef;
use baobao_codegen::{
    builder::{PrimitiveType, TypeMapper as CodeIRTypeMapper},
    language::TypeMapper,
};
use baobao_core::{ArgType, ContextFieldType, DatabaseType};

/// Rust type mapper implementation.
pub struct RustTypeMapper;

/// Rust Code IR type mapper implementation.
///
/// Maps language-agnostic TypeRef types to Rust type syntax.
#[derive(Debug, Clone, Copy, Default)]
pub struct RustCodeTypeMapper;

impl CodeIRTypeMapper for RustCodeTypeMapper {
    fn map_primitive(&self, ty: PrimitiveType) -> String {
        match ty {
            PrimitiveType::String => "String".to_string(),
            PrimitiveType::Int => "i64".to_string(),
            PrimitiveType::UInt => "u64".to_string(),
            PrimitiveType::Float => "f64".to_string(),
            PrimitiveType::Bool => "bool".to_string(),
            PrimitiveType::Path => "std::path::PathBuf".to_string(),
            PrimitiveType::Duration => "std::time::Duration".to_string(),
            PrimitiveType::Char => "char".to_string(),
            PrimitiveType::Byte => "u8".to_string(),
        }
    }

    fn map_optional(&self, inner: &str) -> String {
        format!("Option<{}>", inner)
    }

    fn map_array(&self, inner: &str) -> String {
        format!("Vec<{}>", inner)
    }

    fn map_ref(&self, inner: &str) -> String {
        format!("&{}", inner)
    }

    fn map_ref_mut(&self, inner: &str) -> String {
        format!("&mut {}", inner)
    }

    fn map_result(&self, ok: &str, err: &str) -> String {
        format!("Result<{}, {}>", ok, err)
    }

    fn map_unit(&self) -> String {
        "()".to_string()
    }
}

impl TypeMapper for RustTypeMapper {
    fn language(&self) -> &'static str {
        "rust"
    }

    fn map_arg_type(&self, arg_type: ArgType) -> &'static str {
        match arg_type {
            ArgType::String => "String",
            ArgType::Int => "i64",
            ArgType::Float => "f64",
            ArgType::Bool => "bool",
            ArgType::Path => "std::path::PathBuf",
        }
    }

    fn map_optional_arg_type(&self, arg_type: ArgType) -> String {
        format!("Option<{}>", self.map_arg_type(arg_type))
    }

    fn map_context_type(&self, field_type: &ContextFieldType) -> &'static str {
        match field_type {
            ContextFieldType::Database(DatabaseType::Postgres) => "sqlx::PgPool",
            ContextFieldType::Database(DatabaseType::Mysql) => "sqlx::MySqlPool",
            ContextFieldType::Database(DatabaseType::Sqlite) => "sqlx::SqlitePool",
            ContextFieldType::Http => "reqwest::Client",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_arg_types() {
        let mapper = RustTypeMapper;

        assert_eq!(mapper.map_arg_type(ArgType::String), "String");
        assert_eq!(mapper.map_arg_type(ArgType::Int), "i64");
        assert_eq!(mapper.map_arg_type(ArgType::Float), "f64");
        assert_eq!(mapper.map_arg_type(ArgType::Bool), "bool");
        assert_eq!(mapper.map_arg_type(ArgType::Path), "std::path::PathBuf");
    }

    #[test]
    fn test_rust_optional_types() {
        let mapper = RustTypeMapper;

        assert_eq!(
            mapper.map_optional_arg_type(ArgType::String),
            "Option<String>"
        );
        assert_eq!(mapper.map_optional_arg_type(ArgType::Int), "Option<i64>");
    }

    #[test]
    fn test_rust_context_types() {
        let mapper = RustTypeMapper;

        assert_eq!(
            mapper.map_context_type(&ContextFieldType::Database(DatabaseType::Postgres)),
            "sqlx::PgPool"
        );
        assert_eq!(
            mapper.map_context_type(&ContextFieldType::Database(DatabaseType::Mysql)),
            "sqlx::MySqlPool"
        );
        assert_eq!(
            mapper.map_context_type(&ContextFieldType::Database(DatabaseType::Sqlite)),
            "sqlx::SqlitePool"
        );
        assert_eq!(
            mapper.map_context_type(&ContextFieldType::Http),
            "reqwest::Client"
        );
    }

    #[test]
    fn test_rust_code_type_mapper_primitives() {
        let mapper = RustCodeTypeMapper;

        assert_eq!(mapper.map_primitive(PrimitiveType::String), "String");
        assert_eq!(mapper.map_primitive(PrimitiveType::Int), "i64");
        assert_eq!(mapper.map_primitive(PrimitiveType::UInt), "u64");
        assert_eq!(mapper.map_primitive(PrimitiveType::Float), "f64");
        assert_eq!(mapper.map_primitive(PrimitiveType::Bool), "bool");
        assert_eq!(
            mapper.map_primitive(PrimitiveType::Path),
            "std::path::PathBuf"
        );
        assert_eq!(
            mapper.map_primitive(PrimitiveType::Duration),
            "std::time::Duration"
        );
    }

    #[test]
    fn test_rust_code_type_mapper_complex() {
        let mapper = RustCodeTypeMapper;

        assert_eq!(mapper.map_optional("String"), "Option<String>");
        assert_eq!(mapper.map_array("i64"), "Vec<i64>");
        assert_eq!(mapper.map_ref("String"), "&String");
        assert_eq!(mapper.map_ref_mut("String"), "&mut String");
        assert_eq!(mapper.map_result("()", "Error"), "Result<(), Error>");
        assert_eq!(mapper.map_unit(), "()");
    }

    #[test]
    fn test_rust_code_type_mapper_render() {
        let mapper = RustCodeTypeMapper;

        // Test rendering complex TypeRef
        let string_type = TypeRef::string();
        assert_eq!(mapper.render_type(&string_type), "String");

        let opt_string = TypeRef::optional(TypeRef::string());
        assert_eq!(mapper.render_type(&opt_string), "Option<String>");

        let vec_int = TypeRef::array(TypeRef::int());
        assert_eq!(mapper.render_type(&vec_int), "Vec<i64>");

        let result = TypeRef::result(TypeRef::unit(), TypeRef::named("eyre::Error"));
        assert_eq!(mapper.render_type(&result), "Result<(), eyre::Error>");
    }
}
