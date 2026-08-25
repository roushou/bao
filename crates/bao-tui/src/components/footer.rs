//! The bottom command line: contextual hints, the rm confirmation, prompts,
//! and feedback. Owns the prompt and confirm state.

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    action::Action,
    components::Component,
    event::Event,
    keys::{Keymap, Scope},
    state::{Ctx, Focus, Prompt, PromptAction},
};

pub struct Footer {
    prompt: Option<Prompt>,
    confirm_rm: bool,
}

impl Footer {
    pub fn new() -> Self {
        Footer {
            prompt: None,
            confirm_rm: false,
        }
    }

    pub fn start_prompt(&mut self, label: &'static str, action: PromptAction) {
        self.prompt = Some(Prompt {
            label,
            input: String::new(),
            action,
        });
    }

    pub fn confirm_rm(&mut self) {
        self.confirm_rm = true;
    }

    pub fn has_prompt(&self) -> bool {
        self.prompt.is_some()
    }

    pub fn has_confirm(&self) -> bool {
        self.confirm_rm
    }
}

impl Component for Footer {
    fn handle_events(&mut self, event: Option<&Event>, ctx: &Ctx) -> Action {
        // Remove confirmation.
        if self.confirm_rm {
            if let Some(Event::Key(key)) = event {
                if key.code == KeyCode::Char('y') {
                    self.confirm_rm = false;
                    return ctx
                        .selection
                        .clone()
                        .map(Action::Rm)
                        .unwrap_or(Action::Noop);
                }
            }
            self.confirm_rm = false;
            return Action::Noop;
        }

        // Prompt input.
        if self.prompt.is_some() {
            let Some(Event::Key(key)) = event else {
                return Action::Noop;
            };
            let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
            return match key.code {
                KeyCode::Esc => {
                    self.prompt = None;
                    Action::Noop
                }
                KeyCode::Enter => self.submit_prompt(),
                KeyCode::Backspace => {
                    if let Some(p) = &mut self.prompt {
                        p.input.pop();
                    }
                    Action::Noop
                }
                KeyCode::Char(c) if !ctrl => {
                    if let Some(p) = &mut self.prompt {
                        p.input.push(c);
                    }
                    Action::Noop
                }
                _ => Action::Noop,
            };
        }

        Action::Noop
    }

    fn render(&mut self, f: &mut Frame, ctx: &Ctx, rect: Rect) {
        f.render_widget(Paragraph::new(self.line(ctx, rect.width)), rect);
    }
}

impl Footer {
    fn submit_prompt(&mut self) -> Action {
        let Some(prompt) = self.prompt.take() else {
            return Action::Noop;
        };
        match prompt.action {
            PromptAction::Create => {
                let name = match prompt.input.trim() {
                    "" => None,
                    n => Some(n.to_string()),
                };
                Action::CreateSession(name)
            }
            PromptAction::Rename(id) => {
                let name = prompt.input.trim();
                if name.is_empty() {
                    Action::Noop
                } else {
                    Action::RenameSession(id, Some(name.to_string()))
                }
            }
        }
    }

    /// The browse hints, derived from the keymap so a rebind rewrites the
    /// footer. Pairs collapse ("j/k move").
    fn hints(km: &Keymap) -> String {
        let k = |a: &Action| km.display_of(Scope::Rail, a);
        let pair = |a: &Action, b: &Action, label: &str| match (k(a), k(b)) {
            (Some(x), Some(y)) if x == y => format!("{x} {label}"),
            (Some(x), Some(y)) if x.to_lowercase() == y.to_lowercase() => {
                format!("{x}/{y} {label}")
            }
            _ => String::new(),
        };
        let mut parts = vec![pair(&Action::MoveUp, &Action::MoveDown, "move")];
        if let Some(key) = k(&Action::Open) {
            parts.push(format!("{key} fullscreen"));
        }
        if let Some(key) = k(&Action::FocusTerminal) {
            parts.push(format!("{key} type"));
        }
        if let Some(key) = k(&Action::OpenPalette) {
            parts.push(format!("{key} jump"));
        }
        if let Some(key) = k(&Action::OpenHelp) {
            parts.push(format!("{key} keys"));
        }
        parts
            .iter()
            .filter(|p| !p.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(" · ")
    }

    fn line(&self, ctx: &Ctx, w: u16) -> Line<'static> {
        if ctx.filtering {
            let n = ctx.rows.len();
            let label = if ctx.filter.is_empty() {
                "filter: ".to_string()
            } else {
                format!("filter: {} · {} match", ctx.filter, n)
            };
            return Line::from(Span::styled(
                format!("{label}▌"),
                Style::default().fg(Color::Yellow),
            ));
        }

        if ctx.focus == Focus::Terminal {
            let hint = ctx
                .selection
                .as_ref()
                .and_then(|id| ctx.rows.iter().find(|r| &r.id == id))
                .map(|r| r.action_hint().0)
                .unwrap_or_default();
            return Line::from(vec![
                Span::styled(hint, Style::default().fg(Color::DarkGray)),
                Span::styled(
                    "   ⌃q step out · PgUp/PgDn scroll · ⌃c interrupt",
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
        }

        if ctx.help_open {
            return Line::from(Span::styled(
                "any key closes · ⌃q quit",
                Style::default().fg(Color::DarkGray),
            ));
        }

        if ctx.palette_open {
            return Line::from(Span::styled(
                "↑/↓ move · Enter run · Esc close · ⌃q quit",
                Style::default().fg(Color::DarkGray),
            ));
        }

        let confirm = if self.confirm_rm {
            format!(
                " rm {}? (y/n) ",
                ctx.selection.as_ref().map(|s| s.as_str()).unwrap_or("?")
            )
        } else {
            String::new()
        };

        if let Some(p) = &self.prompt {
            return Line::from(Span::styled(
                format!("{}{}_", p.label, p.input),
                Style::default().fg(Color::Yellow),
            ));
        }

        let feedback = match &ctx.toast {
            Some(m) => Span::styled(format!("» {m}"), Style::default().fg(Color::LightBlue)),
            None => Span::styled(
                ctx.status_line.clone(),
                Style::default().fg(Color::DarkGray),
            ),
        };

        let mut spans = vec![Span::styled(
            Self::hints(Keymap::defaults()),
            Style::default().fg(Color::DarkGray),
        )];
        if !confirm.is_empty() {
            spans.push(Span::styled(confirm, Style::default().fg(Color::Yellow)));
        }
        let left_w: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        let fw = feedback.content.chars().count();
        let pad = (w as usize).saturating_sub(left_w + fw);
        spans.push(Span::raw(" ".repeat(pad)));
        spans.push(feedback);
        Line::from(spans)
    }
}
