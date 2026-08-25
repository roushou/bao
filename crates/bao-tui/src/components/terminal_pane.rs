//! The terminal pane: owns the live terminals (one per session) and which
//! one is shown. Renders the active terminal; forwards raw keys to it.

use std::collections::HashMap;

use bao_client::HostEvent;
use bao_core::types::{SessionId, TerminalSize};
use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::{
    components::Component,
    state::{Ctx, Focus},
    terminal::{Keypress, Terminal},
};

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
                t.draw_in(f, rect, ctx.focus == Focus::Terminal);
            }
        }
    }
}
