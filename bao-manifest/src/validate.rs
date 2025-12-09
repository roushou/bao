//! Validation utilities for Rust identifiers

use miette::SourceSpan;

use crate::{Error, Result};

/// Rust reserved keywords that cannot be used as identifiers
/// Source: https://doc.rust-lang.org/reference/keywords.html
pub(crate) const RUST_KEYWORDS: &[&str] = &[
    // Strict keywords (2021 edition)
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while",
    // Reserved keywords (may be used in future)
    "abstract", "become", "box", "do", "final", "macro", "override", "priv", "try", "typeof",
    "unsized", "virtual", "yield",
    // Weak keywords (context-sensitive, but best to avoid)
    "union", "dyn",
];

/// Check if a name is a Rust reserved keyword
pub(crate) fn is_rust_keyword(name: &str) -> bool {
    RUST_KEYWORDS.contains(&name)
}

/// Find the span of a name in the TOML source
/// Searches for patterns like `.name]`, `.name.`, or `.name =`
pub(crate) fn find_name_span(src: &str, name: &str) -> Option<SourceSpan> {
    // Search for common TOML patterns where the name appears
    let patterns = [
        format!(".{}]", name), // [commands.name] or [commands.parent.name]
        format!(".{}.", name), // [commands.name.something]
        format!(".{} ", name), // inline: name = { ... }
        format!(".{}=", name), // inline without space: name={ ... }
    ];

    for pattern in &patterns {
        if let Some(pos) = src.find(pattern) {
            // +1 to skip the leading dot
            let start = pos + 1;
            let len = name.len();
            return Some(SourceSpan::from((start, len)));
        }
    }

    // Fallback: just find the name anywhere (less precise)
    if let Some(pos) = src.find(name) {
        return Some(SourceSpan::from((pos, name.len())));
    }

    None
}

/// Validate that a name is a valid Rust identifier (or dashed identifier for commands)
/// Returns None if valid, Some(reason) if invalid
///
/// Allows dashes in names (e.g., "my-command") which will be converted to
/// snake_case for Rust identifiers during code generation.
pub(crate) fn validate_identifier(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        return Some("name cannot be empty");
    }

    // Check if it's a reserved keyword (exact match)
    if is_rust_keyword(name) {
        return Some("name is a Rust reserved keyword");
    }

    // Also check if the snake_case version would be a reserved keyword
    let snake_case = name.replace('-', "_");
    if is_rust_keyword(&snake_case) {
        return Some("name converts to a Rust reserved keyword");
    }

    let mut chars = name.chars().peekable();

    // First character must be a letter or underscore
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        Some(_) => return Some("name must start with a letter or underscore"),
        None => return Some("name cannot be empty"),
    }

    let mut prev_was_dash = false;

    // Remaining characters must be alphanumeric, underscore, or dash
    for c in chars {
        if c == '-' {
            if prev_was_dash {
                return Some("name cannot contain consecutive dashes");
            }
            prev_was_dash = true;
        } else if c.is_ascii_alphanumeric() || c == '_' {
            prev_was_dash = false;
        } else {
            return Some("name must contain only letters, numbers, underscores, and dashes");
        }
    }

    // Name cannot end with a dash
    if prev_was_dash {
        return Some("name cannot end with a dash");
    }

    // Names starting with underscore followed by nothing or only underscores are unusual
    // but technically valid, so we allow them

    None
}

/// Validate that a name is a valid Rust identifier, returning an error if invalid
pub(crate) fn validate_name(name: &str, context: &str, src: &str, filename: &str) -> Result<()> {
    let span = find_name_span(src, name);

    if is_rust_keyword(name) {
        return Err(Error::reserved_keyword(name, context, src, filename, span));
    }

    if let Some(reason) = validate_identifier(name) {
        return Err(Error::invalid_identifier(
            name, context, reason, src, filename, span,
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_identifiers() {
        assert!(validate_identifier("hello").is_none());
        assert!(validate_identifier("hello_world").is_none());
        assert!(validate_identifier("HelloWorld").is_none());
        assert!(validate_identifier("_private").is_none());
        assert!(validate_identifier("arg1").is_none());
        assert!(validate_identifier("my_var_2").is_none());
        // Dashed identifiers are now allowed
        assert!(validate_identifier("hello-world").is_none());
        assert!(validate_identifier("my-long-command").is_none());
        assert!(validate_identifier("db-migrate").is_none());
    }

    #[test]
    fn test_reserved_keywords() {
        assert!(validate_identifier("fn").is_some());
        assert!(validate_identifier("struct").is_some());
        assert!(validate_identifier("impl").is_some());
        assert!(validate_identifier("let").is_some());
        assert!(validate_identifier("mut").is_some());
        assert!(validate_identifier("async").is_some());
        assert!(validate_identifier("await").is_some());
        assert!(validate_identifier("self").is_some());
        assert!(validate_identifier("Self").is_some());
        assert!(validate_identifier("type").is_some());
        assert!(validate_identifier("trait").is_some());
        assert!(validate_identifier("enum").is_some());
        assert!(validate_identifier("match").is_some());
        assert!(validate_identifier("mod").is_some());
        assert!(validate_identifier("use").is_some());
        assert!(validate_identifier("pub").is_some());
        assert!(validate_identifier("crate").is_some());
        assert!(validate_identifier("super").is_some());
    }

    #[test]
    fn test_invalid_start_character() {
        assert!(validate_identifier("123abc").is_some());
        assert!(validate_identifier("-name").is_some());
        assert!(validate_identifier("1st").is_some());
    }

    #[test]
    fn test_invalid_characters() {
        assert!(validate_identifier("hello.world").is_some());
        assert!(validate_identifier("hello world").is_some());
        assert!(validate_identifier("hello!").is_some());
        assert!(validate_identifier("name@test").is_some());
    }

    #[test]
    fn test_invalid_dashes() {
        // Dashes at start or end are invalid
        assert!(validate_identifier("-hello").is_some());
        assert!(validate_identifier("hello-").is_some());
        // Consecutive dashes are invalid
        assert!(validate_identifier("hello--world").is_some());
    }

    #[test]
    fn test_dashed_keyword_conversion() {
        // Names that convert to reserved keywords should be rejected
        // "fn_test" is not a keyword, so "fn-test" is allowed
        assert!(validate_identifier("fn-test").is_none());
        // But exact keywords are still rejected
        assert!(validate_identifier("fn").is_some());
    }

    #[test]
    fn test_empty_name() {
        assert!(validate_identifier("").is_some());
    }

    #[test]
    fn test_is_rust_keyword() {
        assert!(is_rust_keyword("fn"));
        assert!(is_rust_keyword("struct"));
        assert!(!is_rust_keyword("hello"));
        assert!(!is_rust_keyword("my_function"));
    }

    #[test]
    fn test_find_name_span() {
        let src = r#"[commands.hello]
description = "test""#;
        let span = find_name_span(src, "hello").unwrap();
        assert_eq!(span.offset(), 10); // Position of 'h' in 'hello'
        assert_eq!(span.len(), 5); // Length of 'hello'
    }

    #[test]
    fn test_find_name_span_nested() {
        let src = r#"[commands.db.commands.migrate]
description = "test""#;
        let span = find_name_span(src, "migrate").unwrap();
        assert_eq!(span.offset(), 22); // Position of 'm' in 'migrate'
        assert_eq!(span.len(), 7); // Length of 'migrate'
    }

    #[test]
    fn test_find_name_span_args() {
        let src = r#"[commands.hello.args.name]
type = "string""#;
        let span = find_name_span(src, "name").unwrap();
        assert_eq!(span.offset(), 21); // Position of 'n' in 'name'
        assert_eq!(span.len(), 4); // Length of 'name'
    }
}
