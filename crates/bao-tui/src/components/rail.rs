//! The sessions rail: selection + filter, and the compact browser. Navigation
//! and verbs happen here; the terminal carries the content.

use bao_core::types::SessionId;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    action::Action,
    components::{Component, pad_trunc},
    event::Event,
    state::{Ctx, Group, Row},
    theme,
};

pub struct Rail {
    selection: Option<SessionId>,
    filter: String,
    filtering: bool,
}

impl Rail {
    pub fn new() -> Self {
        Rail {
            selection: None,
            filter: String::new(),
            filtering: false,
        }
    }

    pub fn selection(&self) -> Option<&SessionId> {
        self.selection.as_ref()
    }

    pub fn set_selection(&mut self, selection: Option<SessionId>) {
        self.selection = selection;
    }

    pub fn filtering(&self) -> bool {
        self.filtering
    }

    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Keep the selection pointing at a live row after rows or the filter
    /// change; fall back to the first visible row.
    pub fn reconcile(&mut self, rows: &[Row]) {
        let ids = self.visible_ids(rows);
        if self
            .selection
            .as_ref()
            .map(|id| ids.contains(id))
            .unwrap_or(false)
        {
            return;
        }
        self.selection = ids.into_iter().next();
    }

    fn visible_ids(&self, rows: &[Row]) -> Vec<SessionId> {
        self.visible_indices(rows)
            .iter()
            .map(|&i| rows[i].id.clone())
            .collect()
    }

    fn visible_indices(&self, rows: &[Row]) -> Vec<usize> {
        if self.filter.is_empty() {
            return (0..rows.len()).collect();
        }
        rows.iter()
            .enumerate()
            .filter(|(_, r)| self.matches(r))
            .map(|(i, _)| i)
            .collect()
    }

    fn matches(&self, row: &Row) -> bool {
        let f = self.filter.to_lowercase();
        if f.is_empty() {
            return true;
        }
        [
            row.name.to_lowercase(),
            row.id.as_str().to_lowercase(),
            row.meta.cwd.display().to_string().to_lowercase(),
            row.harness.to_lowercase(),
        ]
        .iter()
        .any(|h| h.contains(&f))
    }

    fn move_selection(&mut self, rows: &[Row], delta: isize) {
        let ids = self.visible_ids(rows);
        if ids.is_empty() {
            self.selection = None;
            return;
        }
        let pos = self
            .selection
            .as_ref()
            .and_then(|id| ids.iter().position(|x| x == id))
            .unwrap_or(0);
        let new = (pos as isize + delta).clamp(0, ids.len() as isize - 1) as usize;
        self.selection = Some(ids[new].clone());
    }

    fn select_first(&mut self, rows: &[Row]) {
        self.selection = self.visible_ids(rows).into_iter().next();
    }

    fn select_last(&mut self, rows: &[Row]) {
        self.selection = self.visible_ids(rows).into_iter().last();
    }
}

impl Component for Rail {
    fn handle_events(&mut self, event: Option<&Event>, ctx: &Ctx) -> Action {
        // The filter input owns the keyboard when active.
        if self.filtering {
            let Some(Event::Key(key)) = event else {
                return Action::Noop;
            };
            let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
            return match key.code {
                KeyCode::Esc => {
                    self.filter.clear();
                    self.filtering = false;
                    self.reconcile(ctx.rows);
                    Action::Noop
                }
                KeyCode::Enter => {
                    self.filtering = false;
                    self.reconcile(ctx.rows);
                    Action::Noop
                }
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.reconcile(ctx.rows);
                    Action::Noop
                }
                KeyCode::Char(c) if !ctrl => {
                    self.filter.push(c);
                    self.reconcile(ctx.rows);
                    Action::Noop
                }
                _ => Action::Noop,
            };
        }

        let Some(Event::Key(key)) = event else {
            return Action::Noop;
        };
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_selection(ctx.rows, -1);
                Action::Noop
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_selection(ctx.rows, 1);
                Action::Noop
            }
            KeyCode::PageUp => {
                self.move_selection(ctx.rows, -10);
                Action::Noop
            }
            KeyCode::PageDown => {
                self.move_selection(ctx.rows, 10);
                Action::Noop
            }
            KeyCode::Char('g') => {
                self.select_first(ctx.rows);
                Action::Noop
            }
            KeyCode::Char('G') => {
                self.select_last(ctx.rows);
                Action::Noop
            }
            KeyCode::Char('/') => {
                self.filtering = true;
                Action::Noop
            }
            KeyCode::Tab => Action::FocusTerminal,
            KeyCode::Enter => Action::Open,
            KeyCode::Char('?') => Action::OpenHelp,
            KeyCode::Char('p') if ctrl => Action::OpenPalette,
            KeyCode::Char('q') if ctrl => Action::Quit,
            KeyCode::Char('r') => Action::Resume,
            KeyCode::Char('s') => Action::Stop,
            KeyCode::Char('d') => Action::Remove,
            KeyCode::Char('c') => Action::Create,
            KeyCode::Char('n') => Action::Rename,
            _ => Action::Noop,
        }
    }

    fn render(&mut self, f: &mut Frame, ctx: &Ctx, rect: Rect) {
        let vis = self.visible_indices(ctx.rows);
        let needs = vis
            .iter()
            .filter(|&&i| ctx.rows[i].group == Group::NeedsYou)
            .count();

        let mut title = vec![Span::styled(
            format!(" sessions ({}) ", vis.len()),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )];
        if needs > 0 {
            title.push(Span::styled(
                " · ",
                Style::default().fg(theme::palette().dim),
            ));
            title.push(Span::styled(
                needs.to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            ));
            title.push(Span::styled(
                " alert",
                Style::default().fg(theme::palette().dim),
            ));
        }

        let mut lines = vec![Line::from(title)];
        let cw = (rect.width as usize).saturating_sub(2);
        let session_cap = (rect.height as usize).saturating_sub(3);
        for &i in vis.iter() {
            if lines.len() > session_cap {
                break;
            }
            let sel = self.selection.as_ref() == Some(&ctx.rows[i].id);
            lines.push(rail_row(&ctx.rows[i], cw, sel));
        }
        f.render_widget(Paragraph::new(lines), rect);
    }
}

/// One compact rail row.
fn rail_row(row: &Row, cw: usize, sel: bool) -> Line<'static> {
    let name = if row.name.is_empty() {
        row.id.to_string()
    } else {
        row.name.clone()
    };
    let status = row.rail_word();
    let name_w = cw.saturating_sub(5 + status.chars().count()).max(4);
    let status_w = cw.saturating_sub(name_w + 5);

    let fade = if matches!(row.status, bao_core::types::Status::Running) && row.alert.is_none() {
        match row.idle_secs {
            s if s < 20 => Style::default(),
            s if s < 40 => Style::default().fg(Color::Gray),
            _ => Style::default().fg(Color::DarkGray),
        }
    } else {
        row.style()
    };

    let (edge, edge_style) = row.edge_glyph();
    let mut spans = vec![
        Span::styled(edge.to_string(), edge_style),
        Span::styled(" ", Style::default()),
        Span::styled(row.glyph().to_string(), fade),
        Span::styled(" ", Style::default()),
        Span::styled(pad_trunc(&name, name_w), fade.add_modifier(Modifier::BOLD)),
        Span::styled(" ", Style::default().fg(theme::palette().dim)),
        Span::styled(pad_trunc(&status, status_w), fade),
    ];
    if sel {
        for s in &mut spans {
            s.style = s.style.bg(theme::palette().selection);
        }
    }
    Line::from(spans)
}
