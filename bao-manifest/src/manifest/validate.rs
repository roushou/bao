//! Validation context and utilities for manifest parsing.

use std::sync::Arc;

use miette::SourceSpan;

use crate::{Result, error::SourceContext};

/// Parsing and validation context that carries source information.
///
/// This struct encapsulates the source content, filename, and current path
/// through the manifest hierarchy, making it easier to pass validation
/// context through recursive operations.
///
/// # Example
///
/// ```ignore
/// let ctx = ParseContext::new(src, "bao.toml");
/// ctx.validate_name("hello", "command")?;
///
/// // For nested validation
/// let nested = ctx.push("commands").push("db");
/// nested.validate_name("migrate", "subcommand")?;
/// ```
#[derive(Debug, Clone)]
pub struct ParseContext<'a> {
    /// Source context for error reporting (shared across nested contexts)
    source: Arc<SourceContext>,
    /// Path segments for nested validation (e.g., ["commands", "db", "migrate"])
    path: Vec<&'a str>,
}

impl<'a> ParseContext<'a> {
    /// Create a new parse context with the given source and filename.
    pub fn new(src: &str, filename: &str) -> Self {
        Self {
            source: Arc::new(SourceContext::new(src, filename)),
            path: Vec::new(),
        }
    }

    /// Get the source content.
    pub fn src(&self) -> &str {
        self.source.src()
    }

    /// Get the filename.
    pub fn filename(&self) -> &str {
        self.source.filename()
    }

    /// Get the source context for error creation.
    pub fn source_context(&self) -> &SourceContext {
        &self.source
    }

    /// Push a path segment and return a new context.
    ///
    /// This is used when descending into nested structures like subcommands.
    pub fn push(&self, segment: &'a str) -> Self {
        let mut new_path = self.path.clone();
        new_path.push(segment);
        Self {
            source: Arc::clone(&self.source),
            path: new_path,
        }
    }

    /// Get the current path as a dot-separated string.
    ///
    /// Returns the segment if only one element, or joins with dots otherwise.
    /// Returns an empty string if no path segments.
    pub fn path_string(&self) -> String {
        self.path.join(".")
    }

    /// Get a context description for error messages.
    ///
    /// For example: "argument in 'db.migrate'" or just "command" if no path.
    pub fn context_for(&self, kind: &str) -> String {
        if self.path.is_empty() {
            kind.to_string()
        } else {
            format!("{} in '{}'", kind, self.path_string())
        }
    }

    /// Find the span of a name in the source.
    pub fn find_span(&self, name: &str) -> Option<SourceSpan> {
        find_name_span(self.source.src(), name)
    }

    /// Validate that a name is a valid identifier.
    ///
    /// Checks for reserved keywords and valid identifier format.
    pub fn validate_name(&self, name: &str, kind: &str) -> Result<()> {
        if is_rust_keyword(name) {
            return Err(self.source.reserved_keyword_error(
                name,
                self.context_for(kind),
                self.find_span(name),
            ));
        }

        if let Some(reason) = validate_identifier(name) {
            return Err(self.source.invalid_identifier_error(
                name,
                self.context_for(kind),
                reason,
                self.find_span(name),
            ));
        }

        Ok(())
    }
}

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
/// Searches for patterns like `.name]`, `.name.`, `{ name =`, or `name = "value"`
pub(crate) fn find_name_span(src: &str, name: &str) -> Option<SourceSpan> {
    // Search for common TOML patterns where the name appears as an identifier
    // (command name, arg name, flag name)

    // Pattern 1: Table header patterns with leading dot
    // e.g., [commands.name] or [commands.parent.name]
    let patterns_skip_1 = [
        format!(".{}]", name), // [commands.name]
        format!(".{}.", name), // [commands.name.something]
    ];

    for pattern in &patterns_skip_1 {
        if let Some(pos) = src.find(pattern) {
            // +1 to skip the leading dot
            let start = pos + 1;
            return Some(SourceSpan::from((start, name.len())));
        }
    }

    // Pattern 2: Inline table patterns
    // e.g., { name = or , name =
    let inline_patterns = [
        (format!("{{ {} ", name), 2usize), // { name  (brace + space)
        (format!("{{ {}=", name), 2usize), // { name= (brace + space)
        (format!("{{{}=", name), 1usize),  // {name=  (just brace)
        (format!(", {} ", name), 2usize),  // , name  (comma + space)
        (format!(", {}=", name), 2usize),  // , name= (comma + space)
        (format!(",{}=", name), 1usize),   // ,name=  (just comma)
    ];

    for (pattern, skip) in &inline_patterns {
        if let Some(pos) = src.find(pattern) {
            let start = pos + skip;
            return Some(SourceSpan::from((start, name.len())));
        }
    }

    // Pattern 3: Array format with name = "value"
    // e.g., [[commands.hello.args]] followed by name = "target"
    let name_pattern = format!("name = \"{}\"", name);
    if let Some(pos) = src.find(&name_pattern) {
        // The name starts after 'name = "' (8 characters)
        let start = pos + 8;
        return Some(SourceSpan::from((start, name.len())));
    }

    // Also try with single quotes
    let name_pattern_single = format!("name = '{}'", name);
    if let Some(pos) = src.find(&name_pattern_single) {
        let start = pos + 8;
        return Some(SourceSpan::from((start, name.len())));
    }

    // No fallback - better to have no span than point to wrong location
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

    #[test]
    fn test_find_name_span_inline_table() {
        // Test that we find 'type' in inline table, not inside "typescript"
        let src = r#"language = "typescript"
args = { type = { type = "string" } }"#;
        let span = find_name_span(src, "type").unwrap();
        // Should find 'type' at position 33 (args = { type = ...), not position 14 (inside "typescript")
        assert_eq!(span.offset(), 33);
        assert_eq!(span.len(), 4);
    }

    #[test]
    fn test_find_name_span_not_in_string() {
        // Ensure we don't match inside quoted strings
        let src = r#"language = "typescript"
description = "Type your name""#;
        // 'type' appears inside "typescript" and "Type" - neither should match
        let span = find_name_span(src, "type");
        assert!(span.is_none());
    }

    #[test]
    fn test_find_name_span_inline_with_spaces() {
        // Test inline table with spaces after brace
        let src = r#"flags = { verbose = { type = "bool" } }"#;
        let span = find_name_span(src, "verbose").unwrap();
        assert_eq!(span.offset(), 10); // Position of 'v' in 'verbose'
        assert_eq!(span.len(), 7);
    }

    #[test]
    fn test_find_name_span_array_format() {
        // Test array format where name is specified with name = "value"
        let src = r#"[[commands.hello.flags]]
name = "type"
type = "string"
description = "Filter by type""#;
        let span = find_name_span(src, "type").unwrap();
        // Should find 'type' inside name = "type", not at type = "string"
        assert_eq!(span.offset(), 33); // Position of 't' in name = "type"
        assert_eq!(span.len(), 4);
    }

    // ========================================================================
    // ParseContext tests
    // ========================================================================

    #[test]
    fn test_parse_context_new() {
        let ctx = ParseContext::new("content", "bao.toml");
        assert_eq!(ctx.src(), "content");
        assert_eq!(ctx.filename(), "bao.toml");
        assert_eq!(ctx.path_string(), "");
    }

    #[test]
    fn test_parse_context_push() {
        let ctx = ParseContext::new("", "bao.toml");
        let nested = ctx.push("commands").push("db").push("migrate");
        assert_eq!(nested.path_string(), "commands.db.migrate");
    }

    #[test]
    fn test_parse_context_context_for() {
        let ctx = ParseContext::new("", "bao.toml");
        assert_eq!(ctx.context_for("command"), "command");

        let nested = ctx.push("db");
        assert_eq!(nested.context_for("argument"), "argument in 'db'");

        let deep = nested.push("migrate");
        assert_eq!(deep.context_for("flag"), "flag in 'db.migrate'");
    }

    #[test]
    fn test_parse_context_validate_name_valid() {
        let ctx = ParseContext::new("", "bao.toml");
        assert!(ctx.validate_name("hello", "command").is_ok());
        assert!(ctx.validate_name("hello_world", "command").is_ok());
        assert!(ctx.validate_name("hello-world", "command").is_ok());
    }

    #[test]
    fn test_parse_context_validate_name_keyword() {
        let ctx = ParseContext::new("[commands.fn]\ndescription = \"test\"", "bao.toml");
        let result = ctx.validate_name("fn", "command");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("reserved keyword"));
    }

    #[test]
    fn test_parse_context_validate_name_invalid() {
        let ctx = ParseContext::new("", "bao.toml");
        let result = ctx.validate_name("123invalid", "command");
        assert!(result.is_err());
    }
}
