//! Typed Clap attributes for semantic code generation.
//!
//! Instead of passing raw strings like `"command(name = \"foo\")"`, use typed
//! attributes that are rendered to the appropriate syntax.

use std::fmt;

/// Clap attribute for CLI code generation.
///
/// These represent the semantic meaning of clap attributes, which are then
/// rendered to the appropriate syntax during code generation.
#[derive(Debug, Clone, PartialEq)]
pub enum ClapAttr {
    /// `#[command(name = "...")]` - Sets the command name.
    CommandName(String),
    /// `#[command(version = "...")]` - Sets the command version.
    CommandVersion(String),
    /// `#[command(about = "...")]` - Sets the command description.
    CommandAbout(String),
    /// `#[command(subcommand)]` - Marks a field as containing subcommands.
    CommandSubcommand,
    /// `#[arg(...)]` - Marks a field as a CLI argument with options.
    Arg(ArgAttr),
    /// `#[value(name = "...")]` - Sets the value name for enum variants.
    ValueName(String),
}

impl ClapAttr {
    /// Create a command name attribute.
    pub fn command_name(name: impl Into<String>) -> Self {
        Self::CommandName(name.into())
    }

    /// Create a command version attribute.
    pub fn command_version(version: impl Into<String>) -> Self {
        Self::CommandVersion(version.into())
    }

    /// Create a command about attribute.
    pub fn command_about(about: impl Into<String>) -> Self {
        Self::CommandAbout(about.into())
    }

    /// Create a command subcommand attribute.
    pub fn command_subcommand() -> Self {
        Self::CommandSubcommand
    }

    /// Create an arg attribute.
    pub fn arg(attr: ArgAttr) -> Self {
        Self::Arg(attr)
    }

    /// Create a value name attribute.
    pub fn value_name(name: impl Into<String>) -> Self {
        Self::ValueName(name.into())
    }
}

impl fmt::Display for ClapAttr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CommandName(name) => write!(f, "command(name = \"{}\")", name),
            Self::CommandVersion(version) => write!(f, "command(version = \"{}\")", version),
            Self::CommandAbout(about) => write!(f, "command(about = \"{}\")", about),
            Self::CommandSubcommand => write!(f, "command(subcommand)"),
            Self::Arg(attr) => write!(f, "{}", attr),
            Self::ValueName(name) => write!(f, "value(name = \"{}\")", name),
        }
    }
}

/// Options for the `#[arg(...)]` attribute.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ArgAttr {
    /// Include `long` flag (enables `--name` syntax).
    pub long: bool,
    /// Short flag character (enables `-x` syntax).
    pub short: Option<char>,
    /// Default value as a string.
    pub default_value: Option<String>,
}

impl ArgAttr {
    /// Create a new arg attribute builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable long flag.
    pub fn long(mut self) -> Self {
        self.long = true;
        self
    }

    /// Set short flag character.
    pub fn short(mut self, c: char) -> Self {
        self.short = Some(c);
        self
    }

    /// Set default value.
    pub fn default_value(mut self, value: impl Into<String>) -> Self {
        self.default_value = Some(value.into());
        self
    }
}

impl fmt::Display for ArgAttr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();

        if self.long {
            parts.push("long".to_string());
        }
        if let Some(c) = self.short {
            parts.push(format!("short = '{}'", c));
        }
        if let Some(ref default) = self.default_value {
            parts.push(format!("default_value = \"{}\"", default));
        }

        write!(f, "arg({})", parts.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_name() {
        let attr = ClapAttr::command_name("mycli");
        assert_eq!(attr.to_string(), "command(name = \"mycli\")");
    }

    #[test]
    fn test_command_version() {
        let attr = ClapAttr::command_version("1.0.0");
        assert_eq!(attr.to_string(), "command(version = \"1.0.0\")");
    }

    #[test]
    fn test_command_about() {
        let attr = ClapAttr::command_about("A CLI tool");
        assert_eq!(attr.to_string(), "command(about = \"A CLI tool\")");
    }

    #[test]
    fn test_command_subcommand() {
        let attr = ClapAttr::command_subcommand();
        assert_eq!(attr.to_string(), "command(subcommand)");
    }

    #[test]
    fn test_arg_long_only() {
        let attr = ClapAttr::arg(ArgAttr::new().long());
        assert_eq!(attr.to_string(), "arg(long)");
    }

    #[test]
    fn test_arg_long_short() {
        let attr = ClapAttr::arg(ArgAttr::new().long().short('v'));
        assert_eq!(attr.to_string(), "arg(long, short = 'v')");
    }

    #[test]
    fn test_arg_with_default() {
        let attr = ClapAttr::arg(ArgAttr::new().long().default_value("default"));
        assert_eq!(attr.to_string(), "arg(long, default_value = \"default\")");
    }

    #[test]
    fn test_value_name() {
        let attr = ClapAttr::value_name("my-value");
        assert_eq!(attr.to_string(), "value(name = \"my-value\")");
    }
}
