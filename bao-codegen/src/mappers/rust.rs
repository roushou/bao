//! Rust type mapper implementation.

use baobao_core::{ArgType, ContextFieldType, DatabaseType};

use crate::TypeMapper;

/// Rust type mapper implementation.
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
}
