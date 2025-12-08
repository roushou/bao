use std::path::PathBuf;

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

/// Result type for bao-schema operations (boxed to reduce size on stack)
pub type Result<T> = std::result::Result<T, Box<Error>>;

#[derive(Debug, Error, Diagnostic)]
pub enum Error {
    #[error("failed to read '{path}'")]
    #[diagnostic(help("run 'bao init <name>' to create a new project"))]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse bao.toml")]
    #[diagnostic(code(bao::parse_error))]
    Parse {
        #[source_code]
        src: NamedSource<String>,
        #[label("parse error here")]
        span: Option<SourceSpan>,
        #[source]
        source: toml::de::Error,
    },

    #[error("duplicate short flag '-{short}'")]
    #[diagnostic(
        code(bao::duplicate_flag),
        help(
            "choose a different short flag for '{second_flag}', e.g. the first letter of the flag name"
        )
    )]
    DuplicateShortFlag {
        #[source_code]
        src: NamedSource<String>,
        #[label("first used here by '{first_flag}'")]
        first_span: SourceSpan,
        #[label("conflicts with first usage")]
        second_span: SourceSpan,
        short: char,
        first_flag: String,
        second_flag: String,
    },

    #[error("invalid argument type '{ty}'")]
    #[diagnostic(
        code(bao::invalid_type),
        help("valid types are: string, int, float, bool, path")
    )]
    InvalidArgType {
        #[source_code]
        src: NamedSource<String>,
        #[label("unknown type")]
        span: Option<SourceSpan>,
        command: String,
        arg: String,
        ty: String,
    },

    #[error("{message}")]
    #[diagnostic(code(bao::validation_error))]
    Validation {
        #[source_code]
        src: NamedSource<String>,
        #[label("{message}")]
        span: Option<SourceSpan>,
        message: String,
    },

    #[error("'{name}' is a Rust reserved keyword")]
    #[diagnostic(help("rename '{name}' to something else, e.g. '{name}_cmd' or '{name}_arg'"))]
    ReservedKeyword {
        #[source_code]
        src: NamedSource<String>,
        #[label("reserved keyword used here")]
        span: Option<SourceSpan>,
        name: String,
        context: String,
    },

    #[error("invalid {context} name '{name}'")]
    #[diagnostic(help(
        "{reason}. Use only letters, numbers, and underscores, starting with a letter or underscore."
    ))]
    InvalidIdentifier {
        #[source_code]
        src: NamedSource<String>,
        #[label("invalid identifier")]
        span: Option<SourceSpan>,
        name: String,
        context: String,
        reason: String,
    },
}

impl Error {
    /// Create a parse error from a toml error with source context
    pub fn parse(source: toml::de::Error, src: &str, filename: &str) -> Box<Self> {
        let span = source.span().map(SourceSpan::from);
        Box::new(Error::Parse {
            src: NamedSource::new(filename, src.to_string()),
            span,
            source,
        })
    }

    /// Create a validation error with source context
    pub fn validation(message: impl Into<String>, src: &str, filename: &str) -> Box<Self> {
        Box::new(Error::Validation {
            src: NamedSource::new(filename, src.to_string()),
            span: None,
            message: message.into(),
        })
    }

    /// Create a validation error with a span
    pub fn validation_at(
        message: impl Into<String>,
        src: &str,
        filename: &str,
        span: impl Into<SourceSpan>,
    ) -> Box<Self> {
        Box::new(Error::Validation {
            src: NamedSource::new(filename, src.to_string()),
            span: Some(span.into()),
            message: message.into(),
        })
    }

    /// Create a reserved keyword error
    pub fn reserved_keyword(
        name: impl Into<String>,
        context: impl Into<String>,
        src: &str,
        filename: &str,
        span: Option<SourceSpan>,
    ) -> Box<Self> {
        Box::new(Error::ReservedKeyword {
            src: NamedSource::new(filename, src.to_string()),
            span,
            name: name.into(),
            context: context.into(),
        })
    }

    /// Create an invalid identifier error
    pub fn invalid_identifier(
        name: impl Into<String>,
        context: impl Into<String>,
        reason: impl Into<String>,
        src: &str,
        filename: &str,
        span: Option<SourceSpan>,
    ) -> Box<Self> {
        Box::new(Error::InvalidIdentifier {
            src: NamedSource::new(filename, src.to_string()),
            span,
            name: name.into(),
            context: context.into(),
            reason: reason.into(),
        })
    }
}
