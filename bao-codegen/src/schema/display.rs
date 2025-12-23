//! Command tree display formatting.
//!
//! This module provides declarative formatting for command trees,
//! supporting multiple display styles.
//!
//! # Example
//!
//! ```ignore
//! use baobao_codegen::schema::{CommandTree, CommandTreeDisplay, DisplayStyle};
//!
//! let tree = CommandTree::new(&schema);
//! let display = CommandTreeDisplay::new(&tree)
//!     .style(DisplayStyle::WithDescriptions)
//!     .indent("  ");
//!
//! println!("{}", display);
//! ```

use std::fmt;

use super::CommandTree;

/// Display style for command trees.
#[derive(Debug, Clone, Copy, Default)]
pub enum DisplayStyle {
    /// Simple indented names only.
    ///
    /// ```text
    /// hello
    /// users
    ///   create
    ///   delete
    /// ```
    #[default]
    Simple,

    /// Names with descriptions.
    ///
    /// ```text
    /// hello - Say hello
    /// users - User management
    ///   create - Create a user
    ///   delete - Delete a user
    /// ```
    WithDescriptions,

    /// Names with full command signature (args and flags).
    ///
    /// ```text
    /// hello [name] [--loud]
    /// users
    ///   create <username> [--admin]
    ///   delete <id> [--force]
    /// ```
    WithSignature,

    /// Tree structure with box-drawing characters and metadata.
    ///
    /// ```text
    /// ├─ hello (1 arg)
    /// └─ users
    ///    ├─ create (1 arg, 1 flag)
    ///    └─ delete (1 arg)
    /// ```
    TreeBox,
}

/// Declarative command tree display formatter.
#[derive(Debug, Clone)]
pub struct CommandTreeDisplay<'a> {
    tree: &'a CommandTree<'a>,
    style: DisplayStyle,
    indent_str: &'a str,
}

impl<'a> CommandTreeDisplay<'a> {
    /// Create a new display formatter for a command tree.
    pub fn new(tree: &'a CommandTree<'a>) -> Self {
        Self {
            tree,
            style: DisplayStyle::default(),
            indent_str: "  ",
        }
    }

    /// Set the display style.
    pub fn style(mut self, style: DisplayStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the indentation string (default: two spaces).
    pub fn indent(mut self, indent: &'a str) -> Self {
        self.indent_str = indent;
        self
    }

    /// Render the command tree to a string.
    pub fn render(&self) -> String {
        let mut output = String::new();
        self.render_to(&mut output);
        output
    }

    /// Render the command tree to a formatter.
    fn render_to(&self, output: &mut String) {
        // Get top-level commands sorted
        let top_level: Vec<_> = self.tree.iter().filter(|cmd| cmd.depth == 0).collect();

        let mut sorted: Vec<_> = top_level.iter().collect();
        sorted.sort_by_key(|cmd| cmd.name);

        match self.style {
            DisplayStyle::Simple => self.render_simple(output, &sorted),
            DisplayStyle::WithDescriptions => self.render_with_descriptions(output, &sorted),
            DisplayStyle::WithSignature => self.render_with_signature(output, &sorted),
            DisplayStyle::TreeBox => self.render_tree_box(output, &sorted),
        }
    }

    fn render_simple(&self, output: &mut String, commands: &[&&super::FlatCommand<'a>]) {
        for cmd in commands {
            self.render_simple_recursive(output, cmd.name, cmd.command, 0);
        }
    }

    fn render_simple_recursive(
        &self,
        output: &mut String,
        name: &str,
        cmd: &baobao_manifest::Command,
        depth: usize,
    ) {
        // Base indent + depth-based indent
        let indent = format!("{}{}", self.indent_str, self.indent_str.repeat(depth));
        output.push_str(&indent);
        output.push_str(name);
        output.push('\n');

        if cmd.has_subcommands() {
            let mut sorted: Vec<_> = cmd.commands.iter().collect();
            sorted.sort_by_key(|(n, _)| *n);
            for (sub_name, sub_cmd) in sorted {
                self.render_simple_recursive(output, sub_name, sub_cmd, depth + 1);
            }
        }
    }

    fn render_with_descriptions(&self, output: &mut String, commands: &[&&super::FlatCommand<'a>]) {
        for cmd in commands {
            self.render_descriptions_recursive(output, cmd.name, cmd.command, 0);
        }
    }

    fn render_descriptions_recursive(
        &self,
        output: &mut String,
        name: &str,
        cmd: &baobao_manifest::Command,
        depth: usize,
    ) {
        // Base indent + depth-based indent
        let indent = format!("{}{}", self.indent_str, self.indent_str.repeat(depth));
        output.push_str(&indent);
        output.push_str(name);
        output.push_str(" - ");
        output.push_str(&cmd.description);
        output.push('\n');

        if cmd.has_subcommands() {
            let mut sorted: Vec<_> = cmd.commands.iter().collect();
            sorted.sort_by_key(|(n, _)| *n);
            for (sub_name, sub_cmd) in sorted {
                self.render_descriptions_recursive(output, sub_name, sub_cmd, depth + 1);
            }
        }
    }

    fn render_with_signature(&self, output: &mut String, commands: &[&&super::FlatCommand<'a>]) {
        for cmd in commands {
            self.render_signature_recursive(output, cmd.name, cmd.command, 0);
        }
    }

    fn render_signature_recursive(
        &self,
        output: &mut String,
        name: &str,
        cmd: &baobao_manifest::Command,
        depth: usize,
    ) {
        // Base indent + depth-based indent
        let indent = format!("{}{}", self.indent_str, self.indent_str.repeat(depth));
        output.push_str(&indent);

        if cmd.has_subcommands() {
            // Parent command: just name
            output.push_str(name);
            output.push('\n');

            let mut sorted: Vec<_> = cmd.commands.iter().collect();
            sorted.sort_by_key(|(n, _)| *n);
            for (sub_name, sub_cmd) in sorted {
                self.render_signature_recursive(output, sub_name, sub_cmd, depth + 1);
            }
        } else {
            // Leaf command: name with signature
            output.push_str(&Self::format_signature(name, cmd));
            output.push('\n');
        }
    }

    fn format_signature(name: &str, cmd: &baobao_manifest::Command) -> String {
        let mut parts = vec![name.to_string()];

        // Add args (sorted)
        let mut sorted_args: Vec<_> = cmd.args.iter().collect();
        sorted_args.sort_by_key(|(n, _)| *n);
        for (arg_name, arg) in sorted_args {
            if arg.required {
                parts.push(format!("<{}>", arg_name));
            } else {
                parts.push(format!("[{}]", arg_name));
            }
        }

        // Add flags (sorted)
        let mut sorted_flags: Vec<_> = cmd.flags.iter().collect();
        sorted_flags.sort_by_key(|(n, _)| *n);
        let flags: Vec<String> = sorted_flags
            .iter()
            .map(|(flag_name, flag)| {
                if let Some(short) = flag.short_char() {
                    format!("-{}/--{}", short, flag_name)
                } else {
                    format!("--{}", flag_name)
                }
            })
            .collect();

        if !flags.is_empty() {
            parts.push(format!("[{}]", flags.join(" ")));
        }

        parts.join(" ")
    }

    fn render_tree_box(&self, output: &mut String, commands: &[&&super::FlatCommand<'a>]) {
        let total = commands.len();
        for (i, cmd) in commands.iter().enumerate() {
            let is_last = i == total - 1;
            // Start with base indent
            self.render_tree_box_recursive(output, cmd.name, cmd.command, self.indent_str, is_last);
        }
    }

    fn render_tree_box_recursive(
        &self,
        output: &mut String,
        name: &str,
        cmd: &baobao_manifest::Command,
        prefix: &str,
        is_last: bool,
    ) {
        let connector = if is_last { "└─" } else { "├─" };
        let child_prefix = if is_last { "   " } else { "│  " };

        output.push_str(prefix);
        output.push_str(connector);
        output.push(' ');
        output.push_str(name);

        // Add metadata
        let meta = Self::format_metadata(cmd);
        if !meta.is_empty() {
            output.push_str(" (");
            output.push_str(&meta);
            output.push(')');
        }
        output.push('\n');

        // Recurse into subcommands
        if cmd.has_subcommands() {
            let mut sorted: Vec<_> = cmd.commands.iter().collect();
            sorted.sort_by_key(|(n, _)| *n);
            let total = sorted.len();

            for (i, (sub_name, sub_cmd)) in sorted.iter().enumerate() {
                let sub_is_last = i == total - 1;
                let new_prefix = format!("{}{}", prefix, child_prefix);
                self.render_tree_box_recursive(output, sub_name, sub_cmd, &new_prefix, sub_is_last);
            }
        }
    }

    fn format_metadata(cmd: &baobao_manifest::Command) -> String {
        let mut meta = Vec::new();

        if !cmd.args.is_empty() {
            let count = cmd.args.len();
            meta.push(format!(
                "{} arg{}",
                count,
                if count == 1 { "" } else { "s" }
            ));
        }

        if !cmd.flags.is_empty() {
            let count = cmd.flags.len();
            meta.push(format!(
                "{} flag{}",
                count,
                if count == 1 { "" } else { "s" }
            ));
        }

        meta.join(", ")
    }
}

impl fmt::Display for CommandTreeDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let output = self.render();
        // Remove trailing newline for Display
        write!(f, "{}", output.trim_end())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests would require creating a mock Manifest, which is complex.
    // For now, we document the expected behavior.

    #[test]
    fn test_display_style_default() {
        assert!(matches!(DisplayStyle::default(), DisplayStyle::Simple));
    }
}
