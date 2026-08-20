//! Reconstruct a terminal screen as a byte stream, so a fresh client
//! emulator can render the current state without replaying history.
//!
//! The daemon keeps one [`vt100::Parser`] per session — the terminal's
//! *state*, always current — and on attach emits the screen rebuilt as a
//! byte stream that repaints that state in a blank client emulator. This is
//! how tmux and friends attach without replaying the session's whole history:
//! state is transmitted directly, never reconstructed from the log.
//!
//! The rebuild delegates to `vt100`'s own [`vt100::Screen::contents_formatted`],
//! which is correct for attributes, wide characters, the alternate screen,
//! and cursor state by construction — rather than hand-emitting SGR/cursor
//! sequences (which drifted a column on every wide character).

use vt100::Parser;

/// A parser that keeps only the current screen (no scrollback) — the
/// daemon's per-session terminal state.
pub fn parser(rows: u16, cols: u16) -> Parser {
    Parser::new(rows, cols, 0)
}

/// Emit the bytes that rebuild the given screen from scratch. Delegates to
/// `vt100`'s formatter; the result is suitable for feeding to a fresh parser
/// to render the same visual state.
pub fn repaint(p: &Parser) -> Vec<u8> {
    p.screen().contents_formatted()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The screen's text content as a row/col grid — the placement fact that
    /// must survive a snapshot round-trip.
    fn text_grid(p: &Parser) -> Vec<Vec<String>> {
        let s = p.screen();
        let (rows, cols) = s.size();
        (0..rows)
            .map(|r| {
                (0..cols)
                    .map(|c| s.cell(r, c).map(|c| c.contents()).unwrap_or_default())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn repaint_round_trips_a_screen() {
        let mut a = parser(5, 20);
        a.process(b"\x1b[31mred\x1b[0m plain");
        a.process(b"\r\nsecond line");
        let bytes = repaint(&a);

        let mut b = parser(5, 20);
        b.process(&bytes);
        assert_eq!(text_grid(&a), text_grid(&b));
    }

    #[test]
    fn repaint_round_trips_wide_characters() {
        let mut a = parser(3, 20);
        // A wide character spans two cells; a snapshot must not shift the
        // text after it (the hand-rolled repaint did).
        a.process("a好b".as_bytes());
        let bytes = repaint(&a);

        let mut b = parser(3, 20);
        b.process(&bytes);
        assert_eq!(text_grid(&a), text_grid(&b));
    }
}
