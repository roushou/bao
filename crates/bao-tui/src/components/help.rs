//! The keymap overlay: the deep dive behind the contextual hints.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

use crate::{
    action::Action,
    components::{Component, Ctx, pad_trunc},
    event::Event,
};

pub struct Help {
    open: bool,
}

impl Help {
    pub fn new() -> Self {
        Help { open: false }
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }
}

impl Component for Help {
    fn handle_events(&mut self, _event: Option<&Event>, _ctx: &Ctx) -> Action {
        // Any key dismisses it.
        self.open = false;
        Action::Noop
    }

    fn render(&mut self, f: &mut Frame, _ctx: &Ctx, rect: Rect) {
        let cw = (rect.width as usize).saturating_sub(2);
        // Rendered *from* the keymap — the table is the only place keys and
        // labels exist, so this can never drift from what actually routes.
        let km = crate::keys::Keymap::defaults();

        let mut lines: Vec<Line<'static>> = Vec::new();
        for group in crate::keys::Group::ALL {
            lines.push(Line::from(Span::styled(
                pad_trunc(&format!("  {}", group.title()), cw),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )));
            for b in km.bindings(crate::keys::Scope::Rail) {
                if b.group != group {
                    continue;
                }
                let key_w = 12;
                let rest = cw.saturating_sub(key_w + 2);
                lines.push(Line::from(vec![
                    Span::styled(
                        pad_trunc(&b.key.display(), key_w),
                        Style::default().fg(Color::LightBlue),
                    ),
                    Span::styled(" ", Style::default()),
                    Span::styled(
                        pad_trunc(b.label, rest),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
        }
        lines.push(Line::from(Span::styled(
            pad_trunc("  terminal", cw),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )));
        for b in km.bindings(crate::keys::Scope::Terminal) {
            let key_w = 12;
            let rest = cw.saturating_sub(key_w + 2);
            lines.push(Line::from(vec![
                Span::styled(
                    pad_trunc(&b.key.display(), key_w),
                    Style::default().fg(Color::LightBlue),
                ),
                Span::styled(" ", Style::default()),
                Span::styled(
                    pad_trunc(b.label, rest),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
        lines.push(Line::from(Span::styled(
            pad_trunc("    PgUp/PgDn scroll Bao's scrollback", cw),
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            pad_trunc("  palette", cw),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            pad_trunc("    verbs like `resume fix`, `stop db`", cw),
            Style::default().fg(Color::DarkGray),
        )));

        let block = Block::bordered().title(Line::from(Span::styled(
            " help ",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        f.render_widget(Paragraph::new(lines).block(block), rect);
    }
}
