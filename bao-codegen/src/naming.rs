//! Naming conventions for different programming languages.

use baobao_core::{to_camel_case, to_kebab_case, to_pascal_case, to_snake_case};

/// Language-specific naming conventions.
///
/// Defines how to transform command names, field names, and handle reserved words.
#[derive(Debug, Clone, Copy)]
pub struct NamingConvention {
    /// Transform command name to type name (e.g., "hello-world" -> "HelloWorld")
    pub command_to_type: fn(&str) -> String,
    /// Transform command name to file name (e.g., "hello-world" -> "hello_world")
    pub command_to_file: fn(&str) -> String,
    /// Transform field name to language-specific name
    pub field_to_name: fn(&str) -> String,
    /// List of reserved words in the language
    pub reserved_words: &'static [&'static str],
    /// Escape a reserved word (e.g., "type" -> "r#type" in Rust)
    pub escape_reserved: fn(&str) -> String,
}

impl NamingConvention {
    /// Check if a name is a reserved word.
    pub fn is_reserved(&self, name: &str) -> bool {
        self.reserved_words.contains(&name)
    }

    /// Get a safe name, escaping if necessary.
    pub fn safe_name(&self, name: &str) -> String {
        if self.is_reserved(name) {
            (self.escape_reserved)(name)
        } else {
            name.to_string()
        }
    }

    /// Transform and make safe for use as a type name.
    pub fn type_name(&self, name: &str) -> String {
        let transformed = (self.command_to_type)(name);
        self.safe_name(&transformed)
    }

    /// Transform and make safe for use as a file name.
    pub fn file_name(&self, name: &str) -> String {
        // File names typically don't need escaping
        (self.command_to_file)(name)
    }

    /// Transform and make safe for use as a field name.
    pub fn field_name(&self, name: &str) -> String {
        let transformed = (self.field_to_name)(name);
        self.safe_name(&transformed)
    }
}

fn escape_rust_reserved(name: &str) -> String {
    format!("r#{}", name)
}

fn escape_with_underscore(name: &str) -> String {
    format!("_{}", name)
}

/// Rust naming conventions.
pub const RUST_NAMING: NamingConvention = NamingConvention {
    command_to_type: to_pascal_case,
    command_to_file: to_snake_case,
    field_to_name: to_snake_case,
    reserved_words: &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
        "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait",
        "true", "type", "unsafe", "use", "where", "while", "abstract", "become", "box", "do",
        "final", "macro", "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
    ],
    escape_reserved: escape_rust_reserved,
};

/// TypeScript naming conventions.
pub const TYPESCRIPT_NAMING: NamingConvention = NamingConvention {
    command_to_type: to_pascal_case,
    command_to_file: to_kebab_case,
    field_to_name: to_camel_case,
    reserved_words: &[
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
        "as",
        "implements",
        "interface",
        "let",
        "package",
        "private",
        "protected",
        "public",
        "static",
        "yield",
        "any",
        "boolean",
        "constructor",
        "declare",
        "get",
        "module",
        "require",
        "number",
        "set",
        "string",
        "symbol",
        "type",
        "from",
        "of",
        "async",
        "await",
    ],
    escape_reserved: escape_with_underscore,
};

/// Go naming conventions.
pub const GO_NAMING: NamingConvention = NamingConvention {
    command_to_type: to_pascal_case,
    command_to_file: to_snake_case,
    field_to_name: to_pascal_case, // Go uses PascalCase for exported fields
    reserved_words: &[
        "break",
        "case",
        "chan",
        "const",
        "continue",
        "default",
        "defer",
        "else",
        "fallthrough",
        "for",
        "func",
        "go",
        "goto",
        "if",
        "import",
        "interface",
        "map",
        "package",
        "range",
        "return",
        "select",
        "struct",
        "switch",
        "type",
        "var",
    ],
    escape_reserved: escape_with_underscore,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_naming_type() {
        // to_pascal_case splits on underscores
        assert_eq!(RUST_NAMING.type_name("hello_world"), "HelloWorld");
        assert_eq!(RUST_NAMING.type_name("get_user"), "GetUser");
    }

    #[test]
    fn test_rust_naming_file() {
        assert_eq!(RUST_NAMING.file_name("HelloWorld"), "hello_world");
        assert_eq!(RUST_NAMING.file_name("GetUser"), "get_user");
    }

    #[test]
    fn test_rust_naming_field() {
        assert_eq!(RUST_NAMING.field_name("UserName"), "user_name");
        assert_eq!(RUST_NAMING.field_name("userId"), "user_id");
    }

    #[test]
    fn test_rust_reserved_words() {
        assert!(RUST_NAMING.is_reserved("type"));
        assert!(RUST_NAMING.is_reserved("async"));
        assert!(RUST_NAMING.is_reserved("match"));
        assert!(!RUST_NAMING.is_reserved("hello"));
    }

    #[test]
    fn test_rust_escape_reserved() {
        assert_eq!(RUST_NAMING.safe_name("type"), "r#type");
        assert_eq!(RUST_NAMING.safe_name("hello"), "hello");
    }

    #[test]
    fn test_typescript_naming() {
        // to_pascal_case splits on underscores
        assert_eq!(TYPESCRIPT_NAMING.type_name("hello_world"), "HelloWorld");
        assert_eq!(TYPESCRIPT_NAMING.file_name("hello_world"), "hello-world");
        assert_eq!(TYPESCRIPT_NAMING.field_name("user_name"), "userName");
    }

    #[test]
    fn test_typescript_reserved() {
        assert!(TYPESCRIPT_NAMING.is_reserved("class"));
        assert!(TYPESCRIPT_NAMING.is_reserved("interface"));
        assert_eq!(TYPESCRIPT_NAMING.safe_name("type"), "_type");
    }

    #[test]
    fn test_go_naming() {
        // to_pascal_case splits on underscores
        assert_eq!(GO_NAMING.type_name("hello_world"), "HelloWorld");
        assert_eq!(GO_NAMING.field_name("user_name"), "UserName"); // Go exports with PascalCase
    }

    #[test]
    fn test_camel_case() {
        assert_eq!(to_camel_case("hello_world"), "helloWorld");
        assert_eq!(to_camel_case("HelloWorld"), "helloWorld");
        assert_eq!(to_camel_case("get_user_id"), "getUserId");
    }

    #[test]
    fn test_kebab_case() {
        assert_eq!(to_kebab_case("hello_world"), "hello-world");
        assert_eq!(to_kebab_case("HelloWorld"), "hello-world");
    }
}
