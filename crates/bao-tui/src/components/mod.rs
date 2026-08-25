//! The component system: the [`Component`] contract (one struct per surface,
//! each owning its state), its [`Ctx`] snapshot parameter, and the aggregate
//! [`Components`] the overview holds.

pub mod footer;
pub mod header;
pub mod help;
pub mod palette;
pub mod rail;
pub mod tabs;
pub mod terminal_pane;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
};

use bao_core::types::SessionId;

use crate::{
    action::Action,
    event::Event,
    view::{Row, TabView},
};

/// A UI surface: owns its state, renders into a rect, and maps events to
/// cross-cutting [`Action`]s.
pub trait Component {
    fn render(&mut self, f: &mut Frame, ctx: &Ctx, rect: Rect);
    fn handle_events(&mut self, _event: Option<&Event>, _ctx: &Ctx) -> Action {
        Action::Noop
    }
}

/// Shared read-only state handed to components during render. Borrowed from
/// disjoint `Overview` fields so components can be driven mutably alongside
/// it — nothing here is cloned per frame except the rail-owned bits.
pub struct Ctx<'a> {
    pub rows: &'a [Row],
    pub host: &'a str,
    pub focus: Focus,
    pub selection: Option<SessionId>,
    /// The open terminals, in the order they were opened — the tab bar is
    /// their echo, never the navigator.
    pub tabs: &'a [TabView],
    pub filter: String,
    pub filtering: bool,
    pub status_line: &'a str,
    pub toast: Option<&'a str>,
    pub help_open: bool,
    pub palette_open: bool,
}

/// Which pane owns the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Rail,
    Terminal,
}

/// The overview's surfaces.
pub struct Components {
    pub header: header::Header,
    pub rail: rail::Rail,
    pub tabs: tabs::Tabs,
    pub footer: footer::Footer,
    pub palette: palette::Palette,
    pub help: help::Help,
    pub terminal_pane: terminal_pane::TerminalPane,
}

impl Components {
    pub fn new() -> Self {
        Components {
            header: header::Header::new(),
            rail: rail::Rail::new(),
            tabs: tabs::Tabs::new(),
            footer: footer::Footer::new(),
            palette: palette::Palette::new(),
            help: help::Help::new(),
            terminal_pane: terminal_pane::TerminalPane::new(),
        }
    }
}

/// Truncate to `width` chars (ellipsis when cut), then pad to exactly `width`.
pub(crate) fn pad_trunc(s: &str, width: usize) -> String {
    let n = s.chars().count();
    if n > width {
        let keep = width.saturating_sub(1);
        let mut out: String = s.chars().take(keep).collect();
        out.push('…');
        out
    } else {
        let mut out = s.to_string();
        if n < width {
            out.push_str(&" ".repeat(width - n));
        }
        out
    }
}

/// A horizontal rule of exactly `width` cells.
pub(crate) fn rule(width: u16) -> Line<'static> {
    Line::from(Span::styled(
        crate::theme::glyphs()
            .rule
            .to_string()
            .repeat(width as usize),
        Style::default().fg(Color::DarkGray),
    ))
}

/// A vertical hairline of `height` cells.
pub(crate) fn vdivider(height: u16) -> Vec<Line<'static>> {
    let vline = crate::theme::glyphs().vline.to_string();
    let style = Style::default().fg(crate::theme::palette().dim);
    std::iter::repeat_n(Line::styled(vline, style), height as usize).collect()
}
