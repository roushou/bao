//! Naming conventions for different programming languages.

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
