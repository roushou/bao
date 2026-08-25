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
    components::{Component, pad_trunc},
    event::Event,
    state::Ctx,
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
        let groups: [(&str, &[(&str, &str)]); 3] = [
            (
                "navigation",
                &[
                    ("↑/↓ j/k", "move"),
                    ("g / G", "first / last"),
                    ("Enter", "attach selected session"),
                    ("⌃p", "jump — quick switch"),
                    ("/", "filter sessions"),
                    ("?", "this help"),
                ],
            ),
            (
                "actions",
                &[
                    ("c", "create session"),
                    ("r", "resume (interrupted)"),
                    ("s", "stop (running)"),
                    ("n", "rename"),
                    ("d", "remove (confirm)"),
                    ("⌃q", "quit"),
                ],
            ),
            (
                "view",
                &[
                    ("\"", "split — attach in the terminal_pane"),
                    ("palette verbs", "e.g. `resume fix`, `stop db`"),
                ],
            ),
        ];

        let mut lines: Vec<Line<'static>> = Vec::new();
        for (title, keys) in groups.iter() {
            lines.push(Line::from(Span::styled(
                pad_trunc(&format!("  {title}"), cw),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )));
            for (key, what) in keys.iter() {
                let key_w = 12;
                let rest = cw.saturating_sub(key_w + 2);
                lines.push(Line::from(vec![
                    Span::styled(pad_trunc(key, key_w), Style::default().fg(Color::LightBlue)),
                    Span::styled(" ", Style::default()),
                    Span::styled(pad_trunc(what, rest), Style::default().fg(Color::DarkGray)),
                ]));
            }
        }

        let block = Block::bordered().title(Line::from(Span::styled(
            " help ",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        f.render_widget(Paragraph::new(lines).block(block), rect);
    }
}
