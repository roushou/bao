//! The top status strip: brand + one severity cell per session + the machine.
//! Owns only the "newly alert-worthy" flash.

use std::time::{Duration, Instant};

use bao_core::types::SessionId;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    components::{Component, Ctx},
    theme,
};

pub struct Header {
    flash: Option<(SessionId, Instant)>,
}

impl Header {
    pub fn new() -> Self {
        Header { flash: None }
    }

    /// Flash an session's header cell for a beat (it just entered alert).
    pub fn flash(&mut self, id: SessionId) {
        self.flash = Some((id, Instant::now()));
    }

    fn flash_active(&self, id: &SessionId) -> bool {
        match &self.flash {
            Some((pid, t)) if pid == id && t.elapsed() < Duration::from_millis(2000) => {
                (t.elapsed().as_millis() / 250) % 2 == 0
            }
            _ => false,
        }
    }
}

impl Component for Header {
    fn render(&mut self, f: &mut Frame, ctx: &Ctx, rect: Rect) {
        let p = theme::palette();
        let mut spans = vec![Span::styled(
            " Bao ",
            Style::default().add_modifier(Modifier::BOLD),
        )];

        let cap = 40usize;
        for row in ctx.rows.iter().take(cap) {
            let (glyph, style) = row.edge_glyph();
            let st = if self.flash_active(&row.id) {
                style.add_modifier(Modifier::REVERSED)
            } else {
                style
            };
            spans.push(Span::styled(glyph.to_string(), st));
        }
        if ctx.rows.len() > cap {
            spans.push(Span::styled("…", Style::default().fg(p.dim)));
        }
        spans.push(Span::raw(" "));

        let left_w: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        let host = ctx.host.to_string();
        let pad = (rect.width as usize).saturating_sub(left_w + host.chars().count());
        spans.push(Span::raw(" ".repeat(pad)));
        spans.push(Span::styled(host, Style::default().fg(p.dim)));
        f.render_widget(Paragraph::new(Line::from(spans)), rect);
    }
}
