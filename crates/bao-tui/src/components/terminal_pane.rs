//! The terminal pane: owns the live terminals (one per session) and which
//! one is shown. Renders the active terminal; forwards raw keys to it.

use std::collections::HashMap;

use bao_client::HostEvent;
use bao_core::types::{SessionId, TerminalSize};
use crossterm::event::KeyEvent;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::Paragraph,
};

use crate::{
    components::Component,
    components::{Ctx, Focus},
    term::{Keypress, Terminal},
};

/// Render one live terminal inside `rect`: the title bar (signal + name +
/// ended note) over the emulator's screen, with the end-of-session banner
/// laid on top. Presentation only — every decision lives in [`Terminal`].
pub(crate) fn render_terminal(f: &mut Frame, rect: Rect, focused: bool, t: &Terminal) {
    if rect.height < 3 {
        f.render_widget(
            Paragraph::new("terminal too small"),
            Rect::new(rect.x, rect.y, rect.width, 1),
        );
        return;
    }
    let w = rect.width;
    let title_y = rect.y;
    let emu_rect = Rect::new(rect.x, rect.y + 1, w, rect.height.saturating_sub(1));

    let sig = t.signal();
    let name = if t.name.is_empty() {
        t.sid.to_string()
    } else {
        t.name.clone()
    };
    let mut spans = vec![
        Span::styled(
            if focused { "▍ " } else { "  " },
            Style::default().fg(Color::LightBlue),
        ),
        Span::styled(sig.glyph.to_string(), sig.style),
        Span::styled(" ", Style::default()),
        Span::styled(
            format!("{name} · {}", sig.text),
            sig.style.add_modifier(Modifier::BOLD),
        ),
    ];
    if let Some(end) = &t.ended {
        let msg = match end {
            crate::term::End::Exited(code) => format!(
                " — exited{}",
                code.map(|c| format!(" (code {c})")).unwrap_or_default()
            ),
            crate::term::End::Interrupted => {
                " — interrupted (process gone, history kept)".to_string()
            }
            crate::term::End::Gone { reason } => match reason {
                Some(r) => format!(" — launch failed: {r}"),
                None => " — session removed".to_string(),
            },
        };
        spans.push(Span::styled(msg, Style::default().fg(Color::Yellow)));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)),
        Rect::new(rect.x, title_y, w, 1),
    );

    // The emulator is the screen — no borders, no input line.
    f.render_widget(Paragraph::new(Text::from(t.emu.render())), emu_rect);

    // End-of-session banner over the emulator.
    if let Some(end) = &t.ended {
        let msg = match end {
            crate::term::End::Exited(code) => format!(
                " session exited{} — press any key to step out ",
                code.map(|c| format!(" (code {c})")).unwrap_or_default()
            ),
            crate::term::End::Interrupted => {
                " session interrupted — the session process is gone (host restarted); history preserved — press any key to step out "
                    .to_string()
            }
            crate::term::End::Gone { reason } => match reason {
                Some(r) => format!(" launch failed: {r} — press any key to exit "),
                None => " session removed — press any key to exit ".to_string(),
            },
        };
        let banner = Paragraph::new(Text::from(msg)).style(Style::default().fg(Color::Yellow));
        let mid_y = emu_rect.y + emu_rect.height.saturating_sub(1).saturating_sub(2);
        f.render_widget(
            banner,
            Rect::new(emu_rect.x + 1, mid_y, emu_rect.width.saturating_sub(2), 1),
        );
    }
}

pub struct TerminalPane {
    terminals: HashMap<SessionId, Terminal>,
    /// Open order — the tab bar echoes this, never the HashMap's chaos.
    order: Vec<SessionId>,
    active: Option<SessionId>,
    fullscreen: bool,
}

impl TerminalPane {
    pub fn new() -> Self {
        TerminalPane {
            terminals: HashMap::new(),
            order: Vec::new(),
            active: None,
            fullscreen: false,
        }
    }

    pub fn set_active(&mut self, sid: Option<SessionId>) {
        self.active = sid;
    }

    pub fn active(&self) -> Option<&SessionId> {
        self.active.as_ref()
    }

    pub fn fullscreen(&self) -> bool {
        self.fullscreen
    }

    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        self.fullscreen = fullscreen;
    }

    pub fn contains(&self, sid: &SessionId) -> bool {
        self.terminals.contains_key(sid)
    }

    pub fn insert(&mut self, terminal: Terminal) {
        let sid = terminal.sid.clone();
        self.terminals.insert(sid.clone(), terminal);
        if !self.order.contains(&sid) {
            self.order.push(sid);
        }
    }

    pub fn remove(&mut self, sid: &SessionId) {
        self.terminals.remove(sid);
        self.order.retain(|x| x != sid);
    }

    pub fn clear(&mut self) {
        self.terminals.clear();
        self.order.clear();
        self.active = None;
    }

    /// The open terminals in open order (the tab bar's source).
    pub fn open(&self) -> &[SessionId] {
        &self.order
    }

    /// Resize a cached terminal's viewport; returns the new size if it moved.
    pub fn ensure_viewport(
        &mut self,
        sid: &SessionId,
        cols: u16,
        rows: u16,
    ) -> Option<TerminalSize> {
        self.terminals
            .get_mut(sid)
            .and_then(|t| t.set_viewport(cols, rows))
    }

    /// Decide what a keystroke means for the active terminal — purely.
    /// The caller applies effects (send bytes / step out) at the shell.
    pub fn press(&mut self, key: &KeyEvent) -> Option<(SessionId, Keypress)> {
        let sid = self.active.clone()?;
        let kp = self.terminals.get_mut(&sid)?.press(key);
        Some((sid, kp))
    }

    /// Encode a paste for the active terminal — purely.
    pub fn paste_bytes(&self, text: &str) -> Option<(SessionId, Vec<u8>)> {
        let sid = self.active.clone()?;
        let bytes = self.terminals.get(&sid)?.paste_bytes(text);
        Some((sid, bytes))
    }

    pub fn handle_event(&mut self, sid: &SessionId, msg: HostEvent) {
        if let Some(t) = self.terminals.get_mut(sid) {
            t.handle_event(msg);
        }
    }
}

impl Component for TerminalPane {
    // No handle_events: terminal keys are decided by `press` at the dispatch
    // shell (raw passthrough is not a cross-component intent). Only step-out
    // crosses a boundary, and it does so as `Action::StepOut`.

    fn render(&mut self, f: &mut Frame, ctx: &Ctx, rect: Rect) {
        if let Some(sid) = self.active.clone() {
            if let Some(t) = self.terminals.get(&sid) {
                render_terminal(f, rect, ctx.focus == Focus::Terminal, t);
            }
        }
    }
}
