//! The terminal pane: owns the live terminals (one per session) and which
//! one is shown. Renders the active terminal; forwards raw keys to it.

use std::collections::HashMap;

use bao_core::types::{SessionId, TerminalSize};
use bao_wire::client::{ConnWriter, HostMsg};
use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::{
    action::Action,
    components::Component,
    event::Event,
    state::{Ctx, Focus},
    terminal::{Outcome, Terminal},
};

pub struct TerminalPane {
    terminals: HashMap<SessionId, Terminal>,
    active: Option<SessionId>,
    fullscreen: bool,
}

impl TerminalPane {
    pub fn new() -> Self {
        TerminalPane {
            terminals: HashMap::new(),
            active: None,
            fullscreen: false,
        }
    }

    pub fn set_active(&mut self, sid: Option<SessionId>) {
        self.active = sid;
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
        self.terminals.insert(terminal.sid.clone(), terminal);
    }

    pub fn remove(&mut self, sid: &SessionId) {
        self.terminals.remove(sid);
    }

    pub fn clear(&mut self) {
        self.terminals.clear();
        self.active = None;
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

    pub async fn handle_key(&mut self, key: &KeyEvent, writer: &mut ConnWriter) -> Outcome {
        let Some(sid) = self.active.clone() else {
            return Outcome::None;
        };
        match self.terminals.get_mut(&sid) {
            Some(t) => t.handle_key(key, writer).await,
            None => Outcome::None,
        }
    }

    pub fn handle_event(&mut self, sid: &SessionId, msg: HostMsg) {
        if let Some(t) = self.terminals.get_mut(sid) {
            t.handle_event(msg);
        }
    }
}

impl Component for TerminalPane {
    fn handle_events(&mut self, event: Option<&Event>, _ctx: &Ctx) -> Action {
        match event {
            Some(Event::Key(key)) => Action::TerminalKey(*key),
            _ => Action::Noop,
        }
    }

    fn render(&mut self, f: &mut Frame, ctx: &Ctx, rect: Rect) {
        if let Some(sid) = self.active.clone() {
            if let Some(t) = self.terminals.get(&sid) {
                t.draw_in(f, rect, ctx.focus == Focus::Terminal);
            }
        }
    }
}
