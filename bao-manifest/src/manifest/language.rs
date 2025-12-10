//! Language types for code generation.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

/// Supported target languages for code generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// Rust
    Rust,
    /// TypeScript (Bun runtime)
    TypeScript,
}

impl Language {
    /// Returns the language identifier as a static string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Language::Rust => "rust",
            Language::TypeScript => "typescript",
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for Language {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "rust" | "rs" => Ok(Language::Rust),
            "typescript" | "ts" => Ok(Language::TypeScript),
            _ => Err(format!(
                "unknown language '{}', expected 'rust' or 'typescript'",
                s
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_str() {
        assert_eq!(Language::from_str("rust").unwrap(), Language::Rust);
        assert_eq!(Language::from_str("rs").unwrap(), Language::Rust);
        assert_eq!(
            Language::from_str("typescript").unwrap(),
            Language::TypeScript
        );
        assert_eq!(Language::from_str("ts").unwrap(), Language::TypeScript);
        assert_eq!(Language::from_str("Rust").unwrap(), Language::Rust);
        assert_eq!(
            Language::from_str("TypeScript").unwrap(),
            Language::TypeScript
        );
        assert!(Language::from_str("python").is_err());
    }

    #[test]
    fn test_display() {
        assert_eq!(Language::Rust.to_string(), "rust");
        assert_eq!(Language::TypeScript.to_string(), "typescript");
    }

    #[test]
    fn test_deserialize() {
        let rust: Language = serde_json::from_str(r#""rust""#).unwrap();
        assert_eq!(rust, Language::Rust);

        let ts: Language = serde_json::from_str(r#""typescript""#).unwrap();
        assert_eq!(ts, Language::TypeScript);
    }
}
