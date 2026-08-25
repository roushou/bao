//! The tab bar: chrome echoing the open terminals — glyph + title each, the
//! focused one highlighted. It owns no keys; selection lives in the rail
//! (and palette), exactly as the panes contract requires.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{components::Component, state::Ctx, theme};

pub struct Tabs;

impl Tabs {
    pub fn new() -> Self {
        Tabs
    }
}

impl Component for Tabs {
    fn render(&mut self, f: &mut Frame, ctx: &Ctx, rect: Rect) {
        if ctx.tabs.is_empty() {
            return;
        }
        let p = theme::palette();
        let mut spans = vec![Span::styled(" ", Style::default())];
        for (i, tab) in ctx.tabs.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(
                    theme::glyphs().vline.to_string(),
                    Style::default().fg(p.dim),
                ));
            }
            let mut tab_spans = vec![
                Span::styled(" ", Style::default()),
                Span::styled(tab.glyph.to_string(), tab.style),
                Span::styled(" ", Style::default()),
                Span::styled(
                    tab.title.clone(),
                    if tab.active {
                        Style::default().add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(p.dim)
                    },
                ),
                Span::styled(" ", Style::default()),
            ];
            if tab.active {
                for s in &mut tab_spans {
                    s.style = s.style.bg(theme::palette().selection);
                }
            }
            spans.extend(tab_spans);
        }
        // Truncate to width: drop trailing tabs that no longer fit.
        let mut w = 1usize;
        let mut kept: Vec<Span> = Vec::new();
        for s in spans {
            let len = s.content.chars().count();
            if w + len > rect.width as usize {
                break;
            }
            w += len;
            kept.push(s);
        }
        f.render_widget(Paragraph::new(Line::from(kept)), rect);
    }
}
