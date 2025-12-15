//! Path constants for generated files.
//!
//! This module centralizes directory and file paths used by code generators,
//! eliminating magic strings scattered throughout the codebase.

/// Rust-specific paths and constants.
pub mod rust {
    /// Handler files directory relative to project root.
    pub const HANDLERS_DIR: &str = "src/handlers";

    /// Generated commands directory relative to project root.
    pub const COMMANDS_DIR: &str = "src/generated/commands";

    /// Generated module directory relative to project root.
    pub const GENERATED_DIR: &str = "src/generated";

    /// Source directory.
    pub const SRC_DIR: &str = "src";

    /// File extension for Rust source files.
    pub const FILE_EXTENSION: &str = "rs";

    /// Module file name.
    pub const MOD_FILE: &str = "mod.rs";
}

/// TypeScript-specific paths and constants.
pub mod typescript {
    /// Handler files directory relative to project root.
    pub const HANDLERS_DIR: &str = "src/handlers";

    /// Commands directory relative to project root.
    pub const COMMANDS_DIR: &str = "src/commands";

    /// Source directory.
    pub const SRC_DIR: &str = "src";

    /// File extension for TypeScript source files.
    pub const FILE_EXTENSION: &str = "ts";

    /// Index file name.
    pub const INDEX_FILE: &str = "index.ts";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_paths() {
        assert_eq!(rust::HANDLERS_DIR, "src/handlers");
        assert_eq!(rust::COMMANDS_DIR, "src/generated/commands");
        assert_eq!(rust::FILE_EXTENSION, "rs");
    }

    #[test]
    fn test_typescript_paths() {
        assert_eq!(typescript::HANDLERS_DIR, "src/handlers");
        assert_eq!(typescript::COMMANDS_DIR, "src/commands");
        assert_eq!(typescript::FILE_EXTENSION, "ts");
    }
}
