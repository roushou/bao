//! Built-in lints for manifest validation.

mod command_naming;
mod duplicate_command;
mod empty_description;

pub use command_naming::CommandNamingLint;
pub use duplicate_command::DuplicateCommandLint;
pub use empty_description::EmptyDescriptionLint;
