//! The quick-switch palette: a bordered overlay that resolves sessions and
//! verbs from a query. Owns its query + selection.

use bao_core::types::SessionId;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

use crate::{
    action::Action,
    components::{Component, pad_trunc, rule},
    event::Event,
    state::{Ctx, PaletteEntry, Row},
};

pub struct Palette {
    open: bool,
    query: String,
    selected: usize,
}

impl Palette {
    pub fn new() -> Self {
        Palette {
            open: false,
            query: String::new(),
            selected: 0,
        }
    }

    pub fn open_palette(&mut self) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    /// What the current query resolves to.
    pub fn entries(&self, rows: &[Row]) -> Vec<PaletteEntry> {
        self.entries_for(rows, &self.query)
    }

    fn entries_for(&self, rows: &[Row], q: &str) -> Vec<PaletteEntry> {
        let q = q.trim();
        let mut out: Vec<PaletteEntry> = Vec::new();
        if let Some((verb, rest)) = q.split_once(' ') {
            match verb.to_lowercase().as_str() {
                "create" => {
                    out.push(PaletteEntry::Create {
                        name: (!rest.trim().is_empty()).then(|| rest.trim().to_string()),
                    });
                    return out;
                }
                "resume" => {
                    if let Some(id) = best_match(rows, rest) {
                        out.push(PaletteEntry::Action {
                            verb: "resume",
                            glyph: '↻',
                            id,
                        });
                    }
                    return out;
                }
                "stop" => {
                    if let Some(id) = best_match(rows, rest) {
                        out.push(PaletteEntry::Action {
                            verb: "stop",
                            glyph: '⏹',
                            id,
                        });
                    }
                    return out;
                }
                "rm" => {
                    if let Some(id) = best_match(rows, rest) {
                        out.push(PaletteEntry::Action {
                            verb: "remove",
                            glyph: '⚠',
                            id,
                        });
                    }
                    return out;
                }
                "rename" => {
                    if let Some(id) = best_match(rows, rest) {
                        out.push(PaletteEntry::Action {
                            verb: "rename",
                            glyph: '✎',
                            id,
                        });
                    }
                    return out;
                }
                _ => {}
            }
        }
        let mut scored: Vec<(u8, usize)> = rows
            .iter()
            .enumerate()
            .filter_map(|(i, r)| agent_score(r, q).map(|s| (s, i)))
            .collect();
        scored.sort_by_key(|(s, i)| (*s, *i));
        for (_, i) in scored {
            out.push(PaletteEntry::Session(rows[i].id.clone()));
        }
        let lq = q.to_lowercase();
        if q.is_empty() || "create".starts_with(&lq) || out.is_empty() {
            out.push(PaletteEntry::Create { name: None });
        }
        out
    }
}

fn agent_score(r: &Row, q: &str) -> Option<u8> {
    let q = q.trim();
    if q.is_empty() {
        return Some(3);
    }
    let lq = q.to_lowercase();
    let name = r.name.to_lowercase();
    let id = r.id.as_str().to_lowercase();
    let cwd = r.meta.cwd.display().to_string().to_lowercase();
    let harness = r.harness.to_lowercase();
    if name.starts_with(&lq) {
        return Some(0);
    }
    if name.contains(&lq) || id.starts_with(&lq) {
        return Some(1);
    }
    if id.contains(&lq) || cwd.contains(&lq) || harness.contains(&lq) {
        return Some(2);
    }
    None
}

fn best_match(rows: &[Row], q: &str) -> Option<SessionId> {
    if q.trim().is_empty() {
        return None;
    }
    rows.iter()
        .find(|r| agent_score(r, q).is_some())
        .map(|r| r.id.clone())
}

impl Component for Palette {
    fn handle_events(&mut self, event: Option<&Event>, ctx: &Ctx) -> Action {
        let Some(Event::Key(key)) = event else {
            return Action::Noop;
        };
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc | KeyCode::Char('p') if ctrl => {
                self.open = false;
                Action::Noop
            }
            KeyCode::Char(c) if !ctrl => {
                self.query.push(c);
                self.selected = 0;
                Action::Noop
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.selected = 0;
                Action::Noop
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                Action::Noop
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let n = self.entries(ctx.rows).len();
                if n > 0 {
                    self.selected = (self.selected + 1).min(n - 1);
                }
                Action::Noop
            }
            KeyCode::Enter => Action::PaletteConfirm,
            _ => Action::Noop,
        }
    }

    fn render(&mut self, f: &mut Frame, ctx: &Ctx, rect: Rect) {
        let cw = (rect.width as usize).saturating_sub(2);
        let entries = self.entries(ctx.rows);

        let mut lines: Vec<Line<'static>> = vec![Line::from(vec![
            Span::styled(format!("  {}", self.query), Style::default()),
            Span::styled("▌", Style::default().fg(Color::Yellow)),
        ])];
        lines.push(rule(rect.width));

        if entries.is_empty() {
            lines.push(Line::from(Span::styled(
                "  no matches",
                Style::default().fg(Color::DarkGray),
            )));
        }
        for (i, e) in entries.iter().enumerate() {
            let (label, style) = label(ctx.rows, e);
            let mut spans = vec![Span::styled(pad_trunc(&label, cw), style)];
            if i == self.selected {
                for s in &mut spans {
                    s.style = s.style.bg(Color::DarkGray);
                }
            }
            lines.push(Line::from(spans));
        }

        let block = Block::bordered().title(Line::from(Span::styled(
            " jump ",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        f.render_widget(Paragraph::new(lines).block(block), rect);
    }
}

fn label(rows: &[Row], e: &PaletteEntry) -> (String, Style) {
    match e {
        PaletteEntry::Session(id) => {
            if let Some(r) = rows.iter().find(|r| &r.id == id) {
                let name = if r.name.is_empty() {
                    r.id.to_string()
                } else {
                    r.name.clone()
                };
                (
                    format!("{}  {}  {}", r.glyph(), name, r.status_text()),
                    r.style(),
                )
            } else {
                (id.to_string(), Style::default())
            }
        }
        PaletteEntry::Create { name } => {
            let n = name.clone().unwrap_or_default();
            (
                format!("+ create session{n}"),
                Style::default().fg(Color::LightBlue),
            )
        }
        PaletteEntry::Action { verb, glyph, id } => {
            let session = rows
                .iter()
                .find(|r| &r.id == id)
                .map(|r| {
                    if r.name.is_empty() {
                        r.id.to_string()
                    } else {
                        r.name.clone()
                    }
                })
                .unwrap_or_else(|| id.to_string());
            (format!("{glyph} {verb} {session}"), Style::default())
        }
    }
}
