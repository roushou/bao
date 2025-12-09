use std::collections::HashMap;

use super::{Command, Flag};
use crate::{
    error::{Error, Result},
    validate::ParseContext,
};

/// Info about a short flag for error reporting
struct ShortFlagInfo<'a> {
    flag_name: &'a str,
    span: std::ops::Range<usize>,
}

impl Command {
    /// Validate command definition using the given parse context.
    ///
    /// The context tracks the current path through the command hierarchy,
    /// making error messages more informative.
    pub fn validate(&self, ctx: &ParseContext) -> Result<()> {
        // Validate argument names
        for name in self.args.keys() {
            ctx.validate_name(name, "argument")?;
        }

        // Validate flag names and check for duplicate short flags
        let mut short_flags: HashMap<char, ShortFlagInfo> = HashMap::new();

        for (name, flag) in &self.flags {
            // Validate flag name
            ctx.validate_name(name, "flag")?;

            if let Some(ref short) = flag.short {
                let short_char = *short.get_ref();
                let span = short.span();

                if let Some(existing) = short_flags.get(&short_char) {
                    return Err(Box::new(Error::DuplicateShortFlag {
                        src: miette::NamedSource::new(ctx.filename(), ctx.src().to_string()),
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
            ctx.validate_name(name, "subcommand")?;

            // Create a nested context with the subcommand path
            let nested_ctx = ctx.push(name);
            cmd.validate(&nested_ctx)?;
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
