//! Terminal emulation: feeds the session's raw byte stream into a `vt100`
//! parser and renders its screen as ratatui lines. The parser keeps a large
//! scrollback buffer; `set_scroll(offset)` walks up through history.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

const SCROLLBACK: usize = 10_000;

pub struct Emu {
    parser: vt100::Parser,
    cols: u16,
    rows: u16,
}

impl Emu {
    pub fn new(cols: u16, rows: u16) -> Self {
        let (cols, rows) = (cols.clamp(40, 300), rows.clamp(3, 10_000));
        Emu {
            parser: vt100::Parser::new(rows, cols, SCROLLBACK),
            cols,
            rows,
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        let (cols, rows) = (cols.clamp(40, 300), rows.clamp(3, 10_000));
        self.parser.set_size(rows, cols);
        self.cols = cols;
        self.rows = rows;
    }

    pub fn cols(&self) -> u16 {
        self.cols
    }

    pub fn rows(&self) -> u16 {
        self.rows
    }

    /// Scroll offset (0 = live). Applies to the parser; clamps internally.
    pub fn set_scroll(&mut self, offset: usize) {
        self.parser.set_scrollback(offset);
    }

    /// Render the visible screen (which is exactly `rows` tall, with the top
    /// `scroll` rows borrowed from scrollback history).
    pub fn render(&self) -> Vec<Line<'static>> {
        let screen = self.parser.screen();
        let rows = self.rows;
        let cols = self.cols;
        let mut out = Vec::with_capacity(rows as usize);
        for r in 0..rows {
            let mut spans: Vec<Span<'static>> = Vec::with_capacity(cols as usize);
            for c in 0..cols {
                if let Some(cell) = screen.cell(r, c) {
                    let text = cell.contents().to_string();
                    if text.is_empty() {
                        continue;
                    }
                    let mut style = Style::default();
                    let mut fg = to_color(cell.fgcolor());
                    let mut bg = to_color(cell.bgcolor());
                    if cell.inverse() {
                        std::mem::swap(&mut fg, &mut bg);
                    }
                    style = style.fg(fg).bg(bg);
                    if cell.bold() {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    if cell.italic() {
                        style = style.add_modifier(Modifier::ITALIC);
                    }
                    if cell.underline() {
                        style = style.add_modifier(Modifier::UNDERLINED);
                    }
                    spans.push(Span::styled(text, style));
                }
            }
            out.push(Line::from(spans));
        }
        out
    }
}

// Cross-crate `From<vt100::Color>` is blocked by the orphan rule, so this
// stays a private module helper (the honest boundary: private helpers are
// fine; the public surface is typed).
fn to_color(c: vt100::Color) -> Color {
    match c {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(i) => Color::Indexed(i),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}
