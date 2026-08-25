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
    components::Ctx,
    components::{Component, pad_trunc},
    event::Event,
    theme,
    view::{Group, Row, group_rows},
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

    /// Visible sessions in sidebar order: grouped by workspace, groups
    /// flattened. Group headers are display-only — never selectable.
    fn visible_indices(&self, rows: &[Row]) -> Vec<usize> {
        let mut out = Vec::new();
        for group in group_rows(&self.filtered(rows)) {
            for r in &group.rows {
                if let Some(i) = rows.iter().position(|x| x.id == r.id) {
                    out.push(i);
                }
            }
        }
        out
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
            row.command.to_lowercase(),
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

    /// Apply a routed cursor [`Action`]. The keymap owns key→action; the
    /// rail only owns what the cursor does.
    pub fn apply_cursor(&mut self, rows: &[Row], action: &Action) {
        match action {
            Action::MoveUp => self.move_selection(rows, -1),
            Action::MoveDown => self.move_selection(rows, 1),
            Action::PageUp => self.move_selection(rows, -10),
            Action::PageDown => self.move_selection(rows, 10),
            Action::First => self.selection = self.visible_ids(rows).into_iter().next(),
            Action::Last => self.selection = self.visible_ids(rows).into_iter().last(),
            _ => {}
        }
    }

    pub fn start_filter(&mut self) {
        self.filtering = true;
    }
}

impl Component for Rail {
    fn handle_events(&mut self, event: Option<&Event>, ctx: &Ctx) -> Action {
        // Only text entry lives here. Command keys resolve through the
        // keymap in the overview — the table is the single source of truth.
        if !self.filtering {
            return Action::Noop;
        }
        let Some(Event::Key(key)) = event else {
            return Action::Noop;
        };
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
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
        }
    }

    fn render(&mut self, f: &mut Frame, ctx: &Ctx, rect: Rect) {
        let vis = self.visible_indices(ctx.rows);
        let needs = vis
            .iter()
            .filter(|&&i| ctx.rows[i].group == Group::NeedsYou)
            .count();

        let mut title = vec![Span::styled(
            format!(" workspaces ({}) ", vis.len()),
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
        let cap = (rect.height as usize).saturating_sub(3);
        'groups: for group in group_rows(&self.filtered(ctx.rows)) {
            if lines.len() >= cap {
                break;
            }
            // The group header: aggregate state only — glyph + name + count.
            let marker = if group.needs_attention {
                Span::styled(
                    format!("{} ", theme::glyphs().full),
                    Style::default().fg(theme::palette().waiting),
                )
            } else {
                Span::styled(
                    format!("{} ", theme::glyphs().dot),
                    Style::default().fg(theme::palette().dim),
                )
            };
            lines.push(Line::from(vec![
                marker,
                Span::styled(
                    pad_trunc(&group.name, cw.saturating_sub(6)),
                    Style::default()
                        .fg(Color::Gray)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {}", group.rows.len()),
                    Style::default().fg(theme::palette().dim),
                ),
            ]));
            for row in &group.rows {
                if lines.len() >= cap {
                    break 'groups;
                }
                let sel = self.selection.as_ref() == Some(&row.id);
                lines.push(rail_row(row, cw.saturating_sub(1), sel));
            }
        }
        f.render_widget(Paragraph::new(lines), rect);
    }
}

impl Rail {
    /// The rows that survive the filter (grouping input).
    fn filtered(&self, rows: &[Row]) -> Vec<Row> {
        if self.filter.is_empty() {
            rows.to_vec()
        } else {
            rows.iter().filter(|r| self.matches(r)).cloned().collect()
        }
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
