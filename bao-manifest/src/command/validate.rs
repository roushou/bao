use std::collections::HashMap;

use super::{Command, Flag};
use crate::{
    error::{Error, Result},
    validate,
};

/// Info about a short flag for error reporting
pub(super) struct ShortFlagInfo<'a> {
    flag_name: &'a str,
    span: std::ops::Range<usize>,
}

/// Validate that a name is a valid Rust identifier
pub(super) fn validate_name(name: &str, context: &str, src: &str, filename: &str) -> Result<()> {
    let span = validate::find_name_span(src, name);

    if validate::is_rust_keyword(name) {
        return Err(Error::reserved_keyword(name, context, src, filename, span));
    }

    if let Some(reason) = validate::validate_identifier(name) {
        return Err(Error::invalid_identifier(
            name, context, reason, src, filename, span,
        ));
    }

    Ok(())
}

impl Command {
    /// Validate command definition
    pub fn validate(&self, path: &str, src: &str, filename: &str) -> Result<()> {
        // Validate argument names
        for name in self.args.keys() {
            validate_name(name, &format!("argument in '{}'", path), src, filename)?;
        }

        // Validate flag names and check for duplicate short flags
        let mut short_flags: HashMap<char, ShortFlagInfo> = HashMap::new();

        for (name, flag) in &self.flags {
            // Validate flag name
            validate_name(name, &format!("flag in '{}'", path), src, filename)?;

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
            // Validate subcommand name
            validate_name(name, &format!("subcommand in '{}'", path), src, filename)?;
            let nested_path = format!("{}.{}", path, name);
            cmd.validate(&nested_path, src, filename)?;
        }

        Ok(())
    }
}

/// Validation extension for flags
impl Flag {
    /// Get the short flag character, if any
    pub fn short_char(&self) -> Option<char> {
        self.short.as_ref().map(|s| *s.get_ref())
    }
}
