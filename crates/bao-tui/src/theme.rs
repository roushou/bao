//! The token map: the one place colors and glyphs are chosen.
//!
//! Every *meaning* maps to one color here (the semantic palette), and every
//! *shape* maps to one glyph here. The rest of the TUI asks for a meaning or
//! a shape, never a literal color or Unicode char — so the whole interface
//! can be rethemed, turned monochrome (`NO_COLOR`), or dropped to ASCII
//! (`BAO_ASCII=1`, `TERM=dumb`) from exactly one place.

use std::sync::OnceLock;

use ratatui::style::Color;

/// The semantic palette. One color per meaning, resolved once and cached.
#[derive(Clone, Copy)]
pub struct Palette {
    /// The session is blocked on the human (proven by the harness).
    pub waiting: Color,
    /// Exited with a non-zero code.
    pub errored: Color,
    /// Process gone — needs resume or remove.
    pub interrupted: Color,
    /// Quiet past the idle threshold.
    pub idle: Color,
    /// Meta unreadable — needs a human.
    pub damaged: Color,
    /// Running and active — the healthy baseline.
    pub healthy: Color,
    /// Finished cleanly.
    pub done: Color,
    /// Muted context — labels, separators, quiet text.
    pub dim: Color,
    /// The interactive accent — focus, keys, toasts.
    pub accent: Color,
    /// The soft selection background.
    pub selection: Color,
}

static PALETTE: OnceLock<Palette> = OnceLock::new();

/// The active palette, honoring `NO_COLOR` — when set, every meaning is the
/// terminal default and the UI degrades to a monochrome (still legible)
/// form. Cached: the environment is read once per process.
pub fn palette() -> Palette {
    *PALETTE.get_or_init(resolve_palette)
}

fn resolve_palette() -> Palette {
    let off = std::env::var_os("NO_COLOR").is_some();
    Palette {
        waiting: pick(off, Color::Cyan),
        errored: pick(off, Color::Red),
        interrupted: pick(off, Color::Yellow),
        idle: pick(off, Color::Yellow),
        damaged: pick(off, Color::Magenta),
        healthy: pick(off, Color::Green),
        done: pick(off, Color::DarkGray),
        dim: pick(off, Color::DarkGray),
        accent: pick(off, Color::LightBlue),
        selection: pick(off, Color::DarkGray),
    }
}

fn pick(off: bool, color: Color) -> Color {
    if off { Color::Reset } else { color }
}

/// The glyphs the visual language relies on. On a terminal that cannot do
/// Unicode, ASCII stand-ins keep the *shapes* alive, so the severity ladder
/// still reads.
#[derive(Clone, Copy)]
pub struct Glyphs {
    /// Full block — urgent (waiting / errored / damaged / interrupted).
    pub full: char,
    /// Half block — idle.
    pub half: char,
    /// Thin block — running.
    pub thin: char,
    /// Dot — done.
    pub dot: char,
    /// Hollow — preparing/starting (a not-yet-filled running session).
    pub hollow: char,
    /// Horizontal hairline (header and footer borders).
    pub rule: char,
    /// Vertical hairline (rail/terminal divider).
    pub vline: char,
    /// The action-hint arrow.
    pub arrow: char,
}

static GLYPHS: OnceLock<Glyphs> = OnceLock::new();

/// The active glyph set, honoring `BAO_ASCII=1` and `TERM=dumb`.
pub fn glyphs() -> Glyphs {
    *GLYPHS.get_or_init(resolve_glyphs)
}

fn resolve_glyphs() -> Glyphs {
    if ascii() {
        Glyphs {
            full: '#',
            half: '-',
            thin: '|',
            dot: '.',
            hollow: 'o',
            rule: '-',
            vline: '|',
            arrow: '>',
        }
    } else {
        Glyphs {
            full: '█',
            half: '▌',
            thin: '▏',
            dot: '·',
            hollow: '○',
            rule: '─',
            vline: '│',
            arrow: '→',
        }
    }
}

fn ascii() -> bool {
    std::env::var_os("BAO_ASCII").is_some()
        || std::env::var("TERM").map(|t| t == "dumb").unwrap_or(false)
}
