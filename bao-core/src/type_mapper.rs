//! Type mapping between schema types and language-specific types.

use crate::types::{ContextFieldType, DatabaseType};

/// Supported argument types in the schema.
///
/// This is a language-agnostic representation of argument types.
/// Use `TypeMapper` to convert to language-specific type strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgType {
    String,
    Int,
    Float,
    Bool,
    Path,
}

impl ArgType {
    /// Get the schema type name (used in bao.toml)
    pub fn as_str(&self) -> &'static str {
        match self {
            ArgType::String => "string",
            ArgType::Int => "int",
            ArgType::Float => "float",
            ArgType::Bool => "bool",
            ArgType::Path => "path",
        }
    }
}

/// Trait for mapping schema types to language-specific type strings.
///
/// Implement this trait for each target language to provide type mappings.
pub trait TypeMapper {
    /// The target language name
    fn language(&self) -> &'static str;

    /// Map an argument type to a language-specific type string
    fn map_arg_type(&self, arg_type: ArgType) -> &'static str;

    /// Map an optional argument type (e.g., `Option<String>` in Rust, `string | undefined` in TS)
    fn map_optional_arg_type(&self, arg_type: ArgType) -> String {
        // Default implementation - languages can override
        format!("Option<{}>", self.map_arg_type(arg_type))
    }

    /// Map a context field type to a language-specific type string
    fn map_context_type(&self, field_type: &ContextFieldType) -> &'static str;

    /// Map a database type to a language-specific type string
    fn map_database_type(&self, db_type: &DatabaseType) -> &'static str {
        self.map_context_type(&ContextFieldType::Database(*db_type))
    }
}

/// Rust type mapper implementation
pub struct RustTypeMapper;

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
    fn test_rust_type_mapper() {
        let mapper = RustTypeMapper;

        assert_eq!(mapper.map_arg_type(ArgType::String), "String");
        assert_eq!(mapper.map_arg_type(ArgType::Int), "i64");
        assert_eq!(mapper.map_arg_type(ArgType::Float), "f64");
        assert_eq!(mapper.map_arg_type(ArgType::Bool), "bool");
        assert_eq!(mapper.map_arg_type(ArgType::Path), "std::path::PathBuf");

        assert_eq!(
            mapper.map_context_type(&ContextFieldType::Database(DatabaseType::Postgres)),
            "sqlx::PgPool"
        );
        assert_eq!(
            mapper.map_context_type(&ContextFieldType::Http),
            "reqwest::Client"
        );
    }

    #[test]
    fn test_optional_types() {
        let rust = RustTypeMapper;

        assert_eq!(
            rust.map_optional_arg_type(ArgType::String),
            "Option<String>"
        );
    }
}
