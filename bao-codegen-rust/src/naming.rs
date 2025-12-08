//! Rust-specific naming conventions.

use baobao_codegen::NamingConvention;
use baobao_core::{to_pascal_case, to_snake_case};

fn escape_rust_reserved(name: &str) -> String {
    format!("r#{}", name)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_naming_type() {
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
}
