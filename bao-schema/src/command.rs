use std::collections::HashMap;

use serde::{
    Deserialize,
    de::{self, Deserializer, MapAccess, SeqAccess, Visitor},
};
use toml::Spanned;

use crate::error::{Error, Result};

/// A CLI command or subcommand
#[derive(Debug, Deserialize)]
pub struct Command {
    /// Command description for help text
    pub description: String,

    /// Positional arguments
    /// Supports both formats:
    /// - HashMap: `[commands.hello.args.name]` or `args = { name = { type = "string" } }`
    /// - Array: `[[commands.hello.args]]` with `name = "..."` field
    #[serde(default, deserialize_with = "deserialize_args")]
    pub args: HashMap<String, Arg>,

    /// Optional flags
    /// Supports both formats:
    /// - HashMap: `[commands.hello.flags.verbose]` or `flags = { verbose = { short = "v" } }`
    /// - Array: `[[commands.hello.flags]]` with `name = "..."` field
    #[serde(default, deserialize_with = "deserialize_flags")]
    pub flags: HashMap<String, Flag>,

    /// Nested subcommands
    #[serde(default)]
    pub commands: HashMap<String, Command>,
}

/// Info about a short flag for error reporting
struct ShortFlagInfo<'a> {
    flag_name: &'a str,
    span: std::ops::Range<usize>,
}

impl Command {
    /// Validate command definition
    pub fn validate(&self, path: &str, src: &str, filename: &str) -> Result<()> {
        // Check for duplicate short flags
        let mut short_flags: HashMap<char, ShortFlagInfo> = HashMap::new();

        for (name, flag) in &self.flags {
            if let Some(ref short) = flag.short {
                let short_char = *short.get_ref();
                let span = short.span();

                if let Some(existing) = short_flags.get(&short_char) {
                    return Err(Box::new(Error::DuplicateShortFlag {
                        src: miette::NamedSource::new(filename, src.to_string()),
                        first_span: (existing.span.start, existing.span.end - existing.span.start)
                            .into(),
                        second_span: (span.start, span.end - span.start).into(),
                        short: short_char,
                        first_flag: existing.flag_name.to_string(),
                        second_flag: name.clone(),
                    }));
                }

                short_flags.insert(
                    short_char,
                    ShortFlagInfo {
                        flag_name: name,
                        span,
                    },
                );
            }
        }

        // Validate nested commands
        for (name, cmd) in &self.commands {
            let nested_path = format!("{}.{}", path, name);
            cmd.validate(&nested_path, src, filename)?;
        }

        Ok(())
    }

    /// Returns true if this command has subcommands
    pub fn has_subcommands(&self) -> bool {
        !self.commands.is_empty()
    }
}

/// A positional argument
#[derive(Debug, Deserialize)]
pub struct Arg {
    /// Argument type
    #[serde(rename = "type")]
    pub arg_type: ArgType,

    /// Whether the argument is required
    #[serde(default = "default_true")]
    pub required: bool,

    /// Description for help text
    pub description: Option<String>,

    /// Default value (makes argument optional)
    pub default: Option<toml::Value>,
}

fn default_true() -> bool {
    true
}

/// A flag (optional named argument)
#[derive(Debug, Deserialize)]
pub struct Flag {
    /// Flag type
    #[serde(rename = "type", default)]
    pub flag_type: ArgType,

    /// Short flag character (e.g., 'f' for -f)
    /// Wrapped in Spanned to preserve source location for error reporting
    pub short: Option<Spanned<char>>,

    /// Description for help text
    pub description: Option<String>,

    /// Default value
    pub default: Option<toml::Value>,
}

impl Flag {
    /// Get the short flag character, if any
    pub fn short_char(&self) -> Option<char> {
        self.short.as_ref().map(|s| *s.get_ref())
    }
}

/// Supported argument types
#[derive(Debug, Default, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ArgType {
    String,
    Int,
    Float,
    #[default]
    Bool,
    Path,
}

impl ArgType {
    /// Get the string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            ArgType::String => "string",
            ArgType::Int => "int",
            ArgType::Float => "float",
            ArgType::Bool => "bool",
            ArgType::Path => "path",
        }
    }

    /// Get the Rust type for this arg type
    pub fn rust_type(&self) -> &'static str {
        match self {
            ArgType::String => "String",
            ArgType::Int => "i64",
            ArgType::Float => "f64",
            ArgType::Bool => "bool",
            ArgType::Path => "std::path::PathBuf",
        }
    }
}

// ============================================================================
// Custom deserializers for args/flags to support both array and map formats
// ============================================================================

/// Arg with name field for array format deserialization
#[derive(Debug, Deserialize)]
struct ArgWithName {
    name: String,
    #[serde(rename = "type")]
    arg_type: ArgType,
    #[serde(default = "default_true")]
    required: bool,
    description: Option<String>,
    default: Option<toml::Value>,
}

/// Flag with name field for array format deserialization
#[derive(Debug, Deserialize)]
struct FlagWithName {
    name: String,
    #[serde(rename = "type", default)]
    flag_type: ArgType,
    short: Option<char>,
    description: Option<String>,
    default: Option<toml::Value>,
}

/// Untagged enum to support both array and map formats for args
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ArgsFormat {
    Array(Vec<ArgWithName>),
    Map(HashMap<String, Arg>),
}

impl From<ArgsFormat> for HashMap<String, Arg> {
    fn from(format: ArgsFormat) -> Self {
        match format {
            ArgsFormat::Array(vec) => vec
                .into_iter()
                .map(|a| {
                    (
                        a.name.clone(),
                        Arg {
                            arg_type: a.arg_type,
                            required: a.required,
                            description: a.description,
                            default: a.default,
                        },
                    )
                })
                .collect(),
            ArgsFormat::Map(map) => map,
        }
    }
}

fn deserialize_args<'de, D>(deserializer: D) -> std::result::Result<HashMap<String, Arg>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    ArgsFormat::deserialize(deserializer).map(Into::into)
}

/// Deserialize flags from either array or map format
/// Uses manual Visitor because Flag.short uses Spanned which doesn't work with untagged enums
fn deserialize_flags<'de, D>(
    deserializer: D,
) -> std::result::Result<HashMap<String, Flag>, D::Error>
where
    D: Deserializer<'de>,
{
    struct FlagsVisitor;

    impl<'de> Visitor<'de> for FlagsVisitor {
        type Value = HashMap<String, Flag>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a map of flags or an array of flags with name field")
        }

        fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut map = HashMap::new();
            while let Some(item) = seq.next_element::<FlagWithName>()? {
                map.insert(
                    item.name.clone(),
                    Flag {
                        flag_type: item.flag_type,
                        // Use empty span for array format (span info not available)
                        short: item.short.map(|c| Spanned::new(0..0, c)),
                        description: item.description,
                        default: item.default,
                    },
                );
            }
            Ok(map)
        }

        fn visit_map<M>(self, map: M) -> std::result::Result<Self::Value, M::Error>
        where
            M: MapAccess<'de>,
        {
            HashMap::deserialize(de::value::MapAccessDeserializer::new(map))
        }
    }

    deserializer.deserialize_any(FlagsVisitor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Schema;

    fn parse(content: &str) -> Schema {
        toml::from_str(content).expect("Failed to parse TOML")
    }

    // ========================================================================
    // Array format tests
    // ========================================================================

    #[test]
    fn test_args_array_format() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [commands.hello]
            description = "Say hello"

            [[commands.hello.args]]
            name = "target"
            type = "string"
            required = true
            description = "Target to greet"
            "#,
        );

        let cmd = schema.commands.get("hello").unwrap();
        assert_eq!(cmd.args.len(), 1);

        let arg = cmd.args.get("target").unwrap();
        assert_eq!(arg.arg_type, ArgType::String);
        assert!(arg.required);
        assert_eq!(arg.description, Some("Target to greet".to_string()));
    }

    #[test]
    fn test_args_array_format_multiple() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [commands.copy]
            description = "Copy files"

            [[commands.copy.args]]
            name = "source"
            type = "path"
            required = true

            [[commands.copy.args]]
            name = "dest"
            type = "path"
            required = true
            "#,
        );

        let cmd = schema.commands.get("copy").unwrap();
        assert_eq!(cmd.args.len(), 2);
        assert!(cmd.args.contains_key("source"));
        assert!(cmd.args.contains_key("dest"));
        assert_eq!(cmd.args.get("source").unwrap().arg_type, ArgType::Path);
        assert_eq!(cmd.args.get("dest").unwrap().arg_type, ArgType::Path);
    }

    #[test]
    fn test_flags_array_format() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [commands.hello]
            description = "Say hello"

            [[commands.hello.flags]]
            name = "verbose"
            type = "bool"
            short = "v"
            description = "Enable verbose output"
            "#,
        );

        let cmd = schema.commands.get("hello").unwrap();
        assert_eq!(cmd.flags.len(), 1);

        let flag = cmd.flags.get("verbose").unwrap();
        assert_eq!(flag.flag_type, ArgType::Bool);
        assert_eq!(flag.short_char(), Some('v'));
        assert_eq!(flag.description, Some("Enable verbose output".to_string()));
    }

    #[test]
    fn test_flags_array_format_multiple() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [commands.build]
            description = "Build project"

            [[commands.build.flags]]
            name = "release"
            type = "bool"
            short = "r"

            [[commands.build.flags]]
            name = "jobs"
            type = "int"
            short = "j"
            default = 4

            [[commands.build.flags]]
            name = "target"
            type = "string"
            "#,
        );

        let cmd = schema.commands.get("build").unwrap();
        assert_eq!(cmd.flags.len(), 3);

        let release = cmd.flags.get("release").unwrap();
        assert_eq!(release.flag_type, ArgType::Bool);
        assert_eq!(release.short_char(), Some('r'));

        let jobs = cmd.flags.get("jobs").unwrap();
        assert_eq!(jobs.flag_type, ArgType::Int);
        assert_eq!(jobs.short_char(), Some('j'));
        assert!(jobs.default.is_some());

        let target = cmd.flags.get("target").unwrap();
        assert_eq!(target.flag_type, ArgType::String);
        assert_eq!(target.short_char(), None);
    }

    // ========================================================================
    // Map format tests
    // ========================================================================

    #[test]
    fn test_args_map_format() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [commands.hello]
            description = "Say hello"

            [commands.hello.args.target]
            type = "string"
            required = true
            description = "Target to greet"
            "#,
        );

        let cmd = schema.commands.get("hello").unwrap();
        assert_eq!(cmd.args.len(), 1);

        let arg = cmd.args.get("target").unwrap();
        assert_eq!(arg.arg_type, ArgType::String);
        assert!(arg.required);
        assert_eq!(arg.description, Some("Target to greet".to_string()));
    }

    #[test]
    fn test_args_map_format_multiple() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [commands.copy]
            description = "Copy files"

            [commands.copy.args.source]
            type = "path"
            required = true

            [commands.copy.args.dest]
            type = "path"
            required = true
            "#,
        );

        let cmd = schema.commands.get("copy").unwrap();
        assert_eq!(cmd.args.len(), 2);
        assert!(cmd.args.contains_key("source"));
        assert!(cmd.args.contains_key("dest"));
    }

    #[test]
    fn test_flags_map_format() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [commands.hello]
            description = "Say hello"

            [commands.hello.flags.verbose]
            type = "bool"
            short = "v"
            description = "Enable verbose output"
            "#,
        );

        let cmd = schema.commands.get("hello").unwrap();
        assert_eq!(cmd.flags.len(), 1);

        let flag = cmd.flags.get("verbose").unwrap();
        assert_eq!(flag.flag_type, ArgType::Bool);
        assert_eq!(flag.short_char(), Some('v'));
    }

    #[test]
    fn test_flags_map_format_inline() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [commands.hello]
            description = "Say hello"
            flags = { verbose = { type = "bool", short = "v" }, quiet = { type = "bool", short = "q" } }
            "#,
        );

        let cmd = schema.commands.get("hello").unwrap();
        assert_eq!(cmd.flags.len(), 2);
        assert!(cmd.flags.contains_key("verbose"));
        assert!(cmd.flags.contains_key("quiet"));
    }

    // ========================================================================
    // Edge cases and defaults
    // ========================================================================

    #[test]
    fn test_empty_args_and_flags() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [commands.hello]
            description = "Say hello"
            "#,
        );

        let cmd = schema.commands.get("hello").unwrap();
        assert!(cmd.args.is_empty());
        assert!(cmd.flags.is_empty());
    }

    #[test]
    fn test_arg_defaults() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [commands.hello]
            description = "Say hello"

            [[commands.hello.args]]
            name = "name"
            type = "string"
            "#,
        );

        let cmd = schema.commands.get("hello").unwrap();
        let arg = cmd.args.get("name").unwrap();

        // Default: required = true
        assert!(arg.required);
        assert!(arg.description.is_none());
        assert!(arg.default.is_none());
    }

    #[test]
    fn test_flag_defaults() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [commands.hello]
            description = "Say hello"

            [[commands.hello.flags]]
            name = "verbose"
            "#,
        );

        let cmd = schema.commands.get("hello").unwrap();
        let flag = cmd.flags.get("verbose").unwrap();

        // Default: type = bool
        assert_eq!(flag.flag_type, ArgType::Bool);
        assert!(flag.short.is_none());
        assert!(flag.description.is_none());
    }

    #[test]
    fn test_all_arg_types() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [commands.test]
            description = "Test types"

            [[commands.test.args]]
            name = "str_arg"
            type = "string"

            [[commands.test.args]]
            name = "int_arg"
            type = "int"

            [[commands.test.args]]
            name = "float_arg"
            type = "float"

            [[commands.test.args]]
            name = "bool_arg"
            type = "bool"

            [[commands.test.args]]
            name = "path_arg"
            type = "path"
            "#,
        );

        let cmd = schema.commands.get("test").unwrap();
        assert_eq!(cmd.args.get("str_arg").unwrap().arg_type, ArgType::String);
        assert_eq!(cmd.args.get("int_arg").unwrap().arg_type, ArgType::Int);
        assert_eq!(cmd.args.get("float_arg").unwrap().arg_type, ArgType::Float);
        assert_eq!(cmd.args.get("bool_arg").unwrap().arg_type, ArgType::Bool);
        assert_eq!(cmd.args.get("path_arg").unwrap().arg_type, ArgType::Path);
    }

    #[test]
    fn test_arg_with_default_value() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [commands.hello]
            description = "Say hello"

            [[commands.hello.args]]
            name = "count"
            type = "int"
            default = 5
            "#,
        );

        let cmd = schema.commands.get("hello").unwrap();
        let arg = cmd.args.get("count").unwrap();
        assert!(arg.default.is_some());
        assert_eq!(arg.default.as_ref().unwrap().as_integer(), Some(5));
    }

    #[test]
    fn test_subcommands_with_array_format() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [commands.db]
            description = "Database commands"

            [commands.db.commands.migrate]
            description = "Run migrations"

            [[commands.db.commands.migrate.flags]]
            name = "dry_run"
            type = "bool"
            short = "n"

            [commands.db.commands.seed]
            description = "Seed database"

            [[commands.db.commands.seed.args]]
            name = "file"
            type = "path"
            "#,
        );

        let db = schema.commands.get("db").unwrap();
        assert!(db.has_subcommands());
        assert_eq!(db.commands.len(), 2);

        let migrate = db.commands.get("migrate").unwrap();
        assert_eq!(migrate.flags.len(), 1);
        assert!(migrate.flags.contains_key("dry_run"));

        let seed = db.commands.get("seed").unwrap();
        assert_eq!(seed.args.len(), 1);
        assert!(seed.args.contains_key("file"));
    }
}
