//! TypeScript-specific naming conventions.

use baobao_codegen::language::NamingConvention;
use baobao_core::{to_camel_case, to_kebab_case, to_pascal_case};

fn escape_ts_reserved(name: &str) -> String {
    format!("_{}", name)
}

/// TypeScript naming conventions.
pub const TS_NAMING: NamingConvention = NamingConvention {
    // Types use PascalCase
    command_to_type: to_pascal_case,
    // Files use kebab-case
    command_to_file: to_kebab_case,
    // Fields use camelCase
    field_to_name: to_camel_case,
    reserved_words: &[
        // JavaScript reserved words
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "debugger",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "export",
        "extends",
        "false",
        "finally",
        "for",
        "function",
        "if",
        "import",
        "in",
        "instanceof",
        "let",
        "new",
        "null",
        "return",
        "super",
        "switch",
        "this",
        "throw",
        "true",
        "try",
        "typeof",
        "var",
        "void",
        "while",
        "with",
        "yield",
        // TypeScript reserved words
        "any",
        "as",
        "async",
        "await",
        "boolean",
        "constructor",
        "declare",
        "get",
        "implements",
        "interface",
        "module",
        "namespace",
        "never",
        "number",
        "object",
        "package",
        "private",
        "protected",
        "public",
        "readonly",
        "require",
        "set",
        "static",
        "string",
        "symbol",
        "type",
        "undefined",
        "unknown",
    ],
    escape_reserved: escape_ts_reserved,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ts_naming_type() {
        assert_eq!(TS_NAMING.type_name("hello-world"), "HelloWorld");
        assert_eq!(TS_NAMING.type_name("get_user"), "GetUser");
    }

    #[test]
    fn test_ts_naming_file() {
        assert_eq!(TS_NAMING.file_name("HelloWorld"), "hello-world");
        assert_eq!(TS_NAMING.file_name("GetUser"), "get-user");
    }

    #[test]
    fn test_ts_naming_field() {
        assert_eq!(TS_NAMING.field_name("user_name"), "userName");
        assert_eq!(TS_NAMING.field_name("user-id"), "userId");
    }

    #[test]
    fn test_ts_reserved_words() {
        assert!(TS_NAMING.is_reserved("class"));
        assert!(TS_NAMING.is_reserved("async"));
        assert!(TS_NAMING.is_reserved("interface"));
        assert!(!TS_NAMING.is_reserved("hello"));
    }

    #[test]
    fn test_ts_escape_reserved() {
        assert_eq!(TS_NAMING.safe_name("class"), "_class");
        assert_eq!(TS_NAMING.safe_name("hello"), "hello");
    }
}
