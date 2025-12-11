//! Indentation configuration for code generation.

/// Indentation style for generated code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indent {
    /// Spaces with the specified width (e.g., 2 or 4).
    Spaces(u8),
    /// Tab character.
    Tab,
}

impl Indent {
    /// 4-space indentation (Rust, Python).
    pub const RUST: Self = Self::Spaces(4);

    /// 2-space indentation (TypeScript, JavaScript, YAML).
    pub const TYPESCRIPT: Self = Self::Spaces(2);

    /// Tab indentation (Go).
    pub const GO: Self = Self::Tab;

    /// Convert to the string representation for one indent level.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Spaces(2) => "  ",
            Self::Spaces(4) => "    ",
            Self::Spaces(8) => "        ",
            // Fallback to 4 whitespaces
            Self::Spaces(_) => "    ",
            Self::Tab => "\t",
        }
    }
}

impl Default for Indent {
    fn default() -> Self {
        Self::RUST
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indent_as_str() {
        assert_eq!(Indent::Spaces(2).as_str(), "  ");
        assert_eq!(Indent::Spaces(4).as_str(), "    ");
        assert_eq!(Indent::Tab.as_str(), "\t");
    }

    #[test]
    fn test_indent_constants() {
        assert_eq!(Indent::RUST, Indent::Spaces(4));
        assert_eq!(Indent::TYPESCRIPT, Indent::Spaces(2));
        assert_eq!(Indent::GO, Indent::Tab);
    }

    #[test]
    fn test_default() {
        assert_eq!(Indent::default(), Indent::RUST);
    }
}
