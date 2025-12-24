//! Output trait for rendering reports to different formats.

/// Target output for reports.
///
/// Reports describe *what* to output using these semantic methods.
/// Implementations decide *how* to render (terminal, JSON, HTML, etc).
pub trait Output {
    /// Render a title/header.
    fn title(&mut self, text: &str);

    /// Start a new section with a heading.
    fn section(&mut self, name: &str);

    /// Render a key-value pair.
    fn key_value(&mut self, key: &str, value: &str);

    /// Render an indented key-value pair.
    fn key_value_indented(&mut self, key: &str, value: &str);

    /// Render a numbered list item.
    fn numbered_item(&mut self, index: usize, text: &str);

    /// Render a bullet list item.
    fn list_item(&mut self, text: &str);

    /// Render an added item (e.g., new file).
    fn added_item(&mut self, text: &str);

    /// Render a removed item (e.g., deleted file).
    fn removed_item(&mut self, text: &str);

    /// Render a warning message.
    fn warning(&mut self, msg: &str);

    /// Render a separator/divider with a label.
    fn divider(&mut self, label: &str);

    /// Render a block of preformatted text.
    fn preformatted(&mut self, text: &str);

    /// Render a blank line.
    fn newline(&mut self);
}

/// A report that can render itself to an output.
pub trait Report {
    /// Render this report to the given output.
    fn render(&self, out: &mut dyn Output);
}

/// Terminal output implementation.
pub struct TerminalOutput;

impl TerminalOutput {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TerminalOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl Output for TerminalOutput {
    fn title(&mut self, text: &str) {
        println!("{}", text);
        println!("{}", "=".repeat(text.len()));
    }

    fn section(&mut self, name: &str) {
        println!("{}:", name);
    }

    fn key_value(&mut self, key: &str, value: &str) {
        println!("{}: {}", key, value);
    }

    fn key_value_indented(&mut self, key: &str, value: &str) {
        println!("  {}: {}", key, value);
    }

    fn numbered_item(&mut self, index: usize, text: &str) {
        println!("  {}. {}", index, text);
    }

    fn list_item(&mut self, text: &str) {
        println!("  - {}", text);
    }

    fn added_item(&mut self, text: &str) {
        println!("  + {}", text);
    }

    fn removed_item(&mut self, text: &str) {
        println!("  - {}", text);
    }

    fn warning(&mut self, msg: &str) {
        eprintln!("warning: {}", msg);
    }

    fn divider(&mut self, label: &str) {
        println!("── {} ──", label);
    }

    fn preformatted(&mut self, text: &str) {
        println!("{}", text);
    }

    fn newline(&mut self) {
        println!();
    }
}
