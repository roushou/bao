//! Core type definitions.

/// Database type for context fields
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseType {
    Postgres,
    Mysql,
    Sqlite,
}

/// Context field type - language-agnostic representation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextFieldType {
    /// Database connection pool
    Database(DatabaseType),
    /// HTTP client
    Http,
}

impl ContextFieldType {
    /// Returns true if this field type requires async initialization
    pub fn is_async(&self) -> bool {
        matches!(self, ContextFieldType::Database(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_field_type_is_async() {
        assert!(ContextFieldType::Database(DatabaseType::Postgres).is_async());
        assert!(ContextFieldType::Database(DatabaseType::Mysql).is_async());
        assert!(ContextFieldType::Database(DatabaseType::Sqlite).is_async());
        assert!(!ContextFieldType::Http.is_async());
    }
}
