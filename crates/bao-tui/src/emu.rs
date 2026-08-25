//! Terminal emulation: feeds the session's raw byte stream into a `vt100`
//! parser and renders its screen as ratatui lines. The parser keeps a large
//! scrollback buffer; `set_scroll(offset)` walks up through history.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

const SCROLLBACK: usize = 10_000;

/// Terminal modes the harness set on its output stream (DECSET/DECRST).
/// The input encoder honors these — state transmitted by the harness,
/// never guessed by us.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modes {
    /// Application cursor keys (DECCKM, `?1`): arrows/Home/End send SS3.
    pub app_cursor: bool,
    /// Bracketed paste (`?2004`): pastes are wrapped in `ESC [ 200 ~ …`.
    pub bracketed_paste: bool,
}

/// Scan a raw output chunk for private mode set/reset and update `modes`.
/// Recognizes `ESC [ ? <params> h/l`; combined parameters (e.g.
/// `ESC [ ? 1 ; 2004 h`) apply to every param listed. A lightweight sniff —
/// faithful because a real terminal would react to exactly these sequences.
fn scan_modes(bytes: &[u8], modes: &mut Modes) {
    let mut i = 0;
    while i + 3 < bytes.len() {
        if bytes[i] == b'\x1b' && bytes[i + 1] == b'[' && bytes[i + 2] == b'?' {
            let mut j = i + 3;
            let mut params: Vec<u32> = Vec::new();
            let mut cur: Option<u32> = None;
            let end = loop {
                match bytes.get(j) {
                    Some(d) if d.is_ascii_digit() => {
                        cur = Some(cur.unwrap_or(0) * 10 + u32::from(d - b'0'));
                        j += 1;
                    }
                    Some(b';') => {
                        params.push(cur.take().unwrap_or(0));
                        j += 1;
                    }
                    Some(t @ (b'h' | b'l')) => {
                        if let Some(p) = cur.take() {
                            params.push(p);
                        }
                        break Some((t == &b'h', j));
                    }
                    _ => break None, // not a private-mode sequence after all
                }
            };
            if let Some((set, end)) = end {
                for p in params {
                    match p {
                        1 => modes.app_cursor = set,
                        2004 => modes.bracketed_paste = set,
                        _ => {}
                    }
                }
                i = end + 1;
            } else {
                i += 3;
            }
        } else {
            i += 1;
        }
    }
}

pub struct Emu {
    parser: vt100::Parser,
    cols: u16,
    rows: u16,
    modes: Modes,
}

impl Emu {
    pub fn new(cols: u16, rows: u16) -> Self {
        let (cols, rows) = (cols.clamp(40, 300), rows.clamp(3, 10_000));
        Emu {
            parser: vt100::Parser::new(rows, cols, SCROLLBACK),
            cols,
            rows,
            modes: Modes::default(),
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        scan_modes(bytes, &mut self.modes);
        self.parser.process(bytes);
    }

    /// The modes the harness has most recently set on its output.
    pub fn modes(&self) -> Modes {
        self.modes
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_sequences_are_tracked() {
        let mut m = Modes::default();
        scan_modes(b"\x1b[?2004h\x1b[?1h", &mut m);
        assert!(m.app_cursor && m.bracketed_paste);
        scan_modes(b"\x1b[?2004l", &mut m);
        assert!(m.app_cursor && !m.bracketed_paste);
    }

    #[test]
    fn combined_params_apply_individually() {
        let mut m = Modes::default();
        scan_modes(b"\x1b[?1004;2004h", &mut m);
        assert!(!m.app_cursor && m.bracketed_paste);
    }

    #[test]
    fn non_mode_output_is_ignored() {
        let mut m = Modes::default();
        scan_modes(b"hello \x1b[2J world \x1b[?25l text", &mut m);
        assert_eq!(m, Modes::default()); // ?25 is cursor visibility, not ours
        scan_modes(b"\x1b[?20", &mut m); // truncated sequence
        assert_eq!(m, Modes::default());
    }
}
