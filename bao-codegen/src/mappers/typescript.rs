//! TypeScript type mapper implementation.

use baobao_core::{ArgType, ContextFieldType, DatabaseType};

use crate::TypeMapper;

/// TypeScript type mapper implementation.
pub struct TypeScriptTypeMapper;

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
            ContextFieldType::Database(DatabaseType::Postgres) => "Pool",
            ContextFieldType::Database(DatabaseType::Mysql) => "Pool",
            ContextFieldType::Database(DatabaseType::Sqlite) => "Database",
            ContextFieldType::Http => "AxiosInstance",
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
            mapper.map_context_type(&ContextFieldType::Database(DatabaseType::Postgres)),
            "Pool"
        );
        assert_eq!(
            mapper.map_context_type(&ContextFieldType::Database(DatabaseType::Mysql)),
            "Pool"
        );
        assert_eq!(
            mapper.map_context_type(&ContextFieldType::Database(DatabaseType::Sqlite)),
            "Database"
        );
        assert_eq!(
            mapper.map_context_type(&ContextFieldType::Http),
            "AxiosInstance"
        );
    }
}
