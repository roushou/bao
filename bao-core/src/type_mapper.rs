//! Argument type definitions.

/// Supported argument types in the schema.
///
/// This is a language-agnostic representation of argument types.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arg_type_as_str() {
        assert_eq!(ArgType::String.as_str(), "string");
        assert_eq!(ArgType::Int.as_str(), "int");
        assert_eq!(ArgType::Float.as_str(), "float");
        assert_eq!(ArgType::Bool.as_str(), "bool");
        assert_eq!(ArgType::Path.as_str(), "path");
    }
}
