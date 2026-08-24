//! The daemon's terminal screen: the [`vt100::Parser`] plus the policy
//! around it.
//!
//! The daemon keeps one [`Screen`] per session — the terminal's *state*,
//! always current — and on attach emits the screen rebuilt as a byte stream
//! that repaints that state in a blank client emulator. This is how tmux and
//! friends attach without replaying the session's whole history: state is
//! transmitted directly, never reconstructed from the log.
//!
//! The rebuild delegates to `vt100`'s own [`vt100::Screen::contents_formatted`],
//! which is correct for attributes, wide characters, the alternate screen,
//! and cursor state by construction — rather than hand-emitting SGR/cursor
//! sequences (which drifted a column on every wide character).

use bao_core::types::TerminalSize;
use vt100::Parser;

/// One session's terminal state. Wraps a [`vt100::Parser`] configured with
/// **no scrollback** — the daemon transmits current state, never history.
///
/// A plain struct, not a trait: there is exactly one terminal-state
/// implementation, and a port (a second emulator, a wasm target) is
/// speculative until it exists. The daemon's screen *policy* lives here —
/// construction, size mutation, and the attach snapshot contract — so
/// [`Session`](crate::session::Session) owns state, not vt100 trivia.
pub struct Screen {
    parser: Parser,
}

impl Screen {
    /// A fresh screen at the given size, holding no scrollback.
    pub fn new(size: TerminalSize) -> Self {
        Screen {
            parser: Parser::new(size.rows, size.cols, 0),
        }
    }

    /// Feed terminal output into the screen (advances its state only; the
    /// event-log sequence is the caller's to assign).
    pub fn process(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    /// Resize the screen. The PTY window and this parser change together to
    /// the same size — the single size-mutation point for a running session.
    pub fn resize(&mut self, size: TerminalSize) {
        self.parser.set_size(size.rows, size.cols);
    }

    /// The screen's current size, `(rows, cols)` — what a client emulator
    /// must render at.
    pub fn size(&self) -> (u16, u16) {
        self.parser.screen().size()
    }

    /// Emit the bytes that rebuild this screen from scratch. Delegates to
    /// `vt100`'s formatter; the result is suitable for feeding to a fresh
    /// parser to render the same visual state.
    pub fn repaint(&self) -> Vec<u8> {
        self.parser.screen().contents_formatted()
    }

    /// A consistent (log sequence, screen snapshot) pair for attach: the
    /// snapshot reflects exactly the output with `seq <=` the returned
    /// value, so replaying the log from that sequence reproduces everything
    /// the snapshot doesn't already show — no loss, no duplication. The
    /// caller must hold this screen's lock while assigning the sequence for
    /// output (as [`Session::append`](crate::session::Session) does), which
    /// is what makes the pair consistent.
    pub fn snapshot(&self, seq: u64) -> (u64, Vec<u8>) {
        (seq, self.repaint())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The screen's text content as a row/col grid — the placement fact that
    /// must survive a snapshot round-trip.
    fn text_grid(s: &Screen) -> Vec<Vec<String>> {
        let v = s.parser.screen();
        let (rows, cols) = v.size();
        (0..rows)
            .map(|r| {
                (0..cols)
                    .map(|c| v.cell(r, c).map(|c| c.contents()).unwrap_or_default())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn repaint_round_trips_a_screen() {
        let mut a = Screen::new(TerminalSize { rows: 5, cols: 20 });
        a.process(b"\x1b[31mred\x1b[0m plain");
        a.process(b"\r\nsecond line");
        let bytes = a.repaint();

        let mut b = Screen::new(TerminalSize { rows: 5, cols: 20 });
        b.process(&bytes);
        assert_eq!(text_grid(&a), text_grid(&b));
    }

    #[test]
    fn repaint_round_trips_wide_characters() {
        let mut a = Screen::new(TerminalSize { rows: 3, cols: 20 });
        // A wide character spans two cells; a snapshot must not shift the
        // text after it (the hand-rolled repaint did).
        a.process("a好b".as_bytes());
        let bytes = a.repaint();

        let mut b = Screen::new(TerminalSize { rows: 3, cols: 20 });
        b.process(&bytes);
        assert_eq!(text_grid(&a), text_grid(&b));
    }

    #[test]
    fn snapshot_pairs_the_screen_with_the_given_seq() {
        let s = Screen::new(TerminalSize { rows: 5, cols: 20 });
        assert_eq!(s.snapshot(7).0, 7);
    }
}
