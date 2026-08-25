//! The Terminal pane: the running harness, natively.
//!
//! The terminal is a raw PTY attach — a dumb byte pipe in both directions.
//! Output: the daemon's byte stream feeds a real vt100 emulator; the harness
//! draws its own screen, echoes your typing, edits its own lines. Input: raw
//! — each keystroke is encoded to the bytes the terminal would send and
//! forwarded immediately. There is no line buffer, no Bao-side echo, no
//! "send on Enter": the harness's own input machinery is the truth.
//!
//! One reserved key, `⌃q`, steps out (and is never forwarded). `PgUp/PgDn`
//! scroll Bao's scrollback, so the arrows stay with the harness.

use bao_client::{ConnWriter, HostEvent};
use bao_core::types::{SessionId, SessionMeta, Status, TerminalSize};
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent};
use futures_util::StreamExt;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::Paragraph,
};
use tokio::sync::mpsc;

use crate::{emu::Emu, error::Error, signal};

/// What one keypress means to the terminal model — decided purely (no I/O,
/// no connection), applied by the caller at the shell. `StepOut` crosses a
/// pane boundary and maps to [`crate::action::Action::StepOut`] there; the
/// other variants stay inside the terminal subsystem.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Keypress {
    /// Leave the terminal: the keymap's reserved key, or any key once the
    /// session has ended.
    StepOut,
    /// Scroll Bao's scrollback by lines — already applied to local state;
    /// the caller does nothing.
    Scroll(isize),
    /// Bytes to deliver to the harness's stdin.
    Send(Vec<u8>),
    /// Not a key a terminal would send — drop it.
    Ignore,
}

/// The session is over, one way or another — the TUI shows a banner and any
/// key steps out.
#[derive(Clone, Debug, PartialEq)]
pub enum End {
    Exited(Option<i32>),
    /// Session process is gone (host restarted / machine rebooted), history kept.
    Interrupted,
    /// The session was removed — a rolled-back launch, or an `rm`.
    Gone {
        reason: Option<String>,
    },
}

impl End {
    fn from_status(status: Status) -> Option<End> {
        match status {
            Status::Exited(code) => Some(End::Exited(code)),
            Status::Interrupted => Some(End::Interrupted),
            _ => None,
        }
    }
}

pub struct Terminal {
    pub sid: SessionId,
    pub name: String,
    pub emu: Emu,
    pub scroll: usize,
    /// The latest lifecycle status (from the snapshot or the state stream).
    pub status: Status,
    /// The latest full picture — drives the signal (alert, waiting,
    /// idle) in the title.
    pub meta: Option<SessionMeta>,
    pub ended: Option<End>,
}

impl Terminal {
    /// Rows the emulator renders: the pane height minus the one-line title
    /// bar, clamped to the emulator's legal range.
    fn viewport_rows(rows: u16) -> u16 {
        rows.saturating_sub(1).clamp(3, 10_000)
    }

    pub fn new(sid: SessionId, meta: Option<SessionMeta>, cols: u16, rows: u16) -> Self {
        let mut t = Terminal {
            sid,
            name: String::new(),
            emu: Emu::new(cols, Self::viewport_rows(rows)),
            scroll: 0,
            status: Status::Running,
            meta: None,
            ended: None,
        };
        t.set_meta(meta);
        t
    }

    /// Apply a session's current picture (drives the title signal and the
    /// ended banner). `None` = not known yet (before the attach reply).
    pub fn set_meta(&mut self, meta: Option<SessionMeta>) {
        self.name = meta
            .as_ref()
            .and_then(|m| m.name.clone())
            .unwrap_or_default();
        self.status = meta.as_ref().map(|m| m.status).unwrap_or(Status::Running);
        self.ended = meta.as_ref().and_then(|m| End::from_status(m.status));
        self.meta = meta;
    }

    /// The exact size the emulator renders (post-clamp). This is the size the
    /// daemon's PTY must match — the single source of truth for the pane.
    pub fn viewport_size(&self) -> TerminalSize {
        TerminalSize {
            cols: self.emu.cols(),
            rows: self.emu.rows(),
        }
    }

    fn signal(&self) -> signal::Signal {
        let (alert, waiting, idle) = match &self.meta {
            Some(m) => (m.alert, m.waiting_for_input == Some(true), m.idle_secs),
            None => (None, false, 0),
        };
        signal::signal(self.status, alert, waiting, idle)
    }

    pub fn handle_event(&mut self, msg: HostEvent) {
        match msg {
            HostEvent::Output { data, .. } => self.emu.feed(&data),
            HostEvent::Status { status, .. } => {
                self.status = status;
                self.ended = End::from_status(status);
            }
            HostEvent::State { meta, .. } => {
                self.status = meta.status;
                self.ended = End::from_status(meta.status);
                self.meta = Some(meta);
            }
            HostEvent::Gone { reason, .. } => {
                self.ended = Some(End::Gone { reason });
            }
            HostEvent::Disconnected => self.ended = Some(End::Interrupted),
        }
    }

    /// Decide what a keystroke means — purely: no connection, no awaiting.
    /// Raw passthrough dialect: `⌃q` (the keymap's one reserved key) steps
    /// out, `PgUp/PgDn` scroll Bao's scrollback (applied to local state here),
    /// everything else encodes to the bytes a terminal would send. The caller
    /// applies the effect at the shell.
    pub fn press(&mut self, k: &KeyEvent) -> Keypress {
        if self.ended.is_some() {
            return Keypress::StepOut;
        }
        // Step-out is the table's one exception to passthrough — resolved
        // here so the attached view and the overview share it.
        if crate::keys::Keymap::defaults()
            .resolve(crate::keys::Scope::Terminal, k)
            .is_some()
        {
            return Keypress::StepOut;
        }
        match k.code {
            KeyCode::PageUp => {
                self.scroll += 12;
                self.emu.set_scroll(self.scroll);
                Keypress::Scroll(12)
            }
            KeyCode::PageDown => {
                self.scroll = self.scroll.saturating_sub(12);
                self.emu.set_scroll(self.scroll);
                Keypress::Scroll(-12)
            }
            _ => {
                let modes = self.emu.modes();
                match crate::input::Encoder::new(&modes).key(k) {
                    Some(bytes) => Keypress::Send(bytes),
                    None => Keypress::Ignore,
                }
            }
        }
    }

    /// Encode a paste, honoring the harness's bracketed-paste mode (wrapped
    /// iff it asked for it). Pure; the caller sends the bytes.
    pub fn paste_bytes(&self, text: &str) -> Vec<u8> {
        let modes = self.emu.modes();
        crate::input::Encoder::new(&modes).paste(text)
    }

    /// Resize the emulator's viewport. Returns the new size when it actually
    /// changed (post-clamp), so the caller can propagate it to the daemon's
    /// PTY — and skip the round-trip when nothing moved.
    pub fn set_viewport(&mut self, cols: u16, rows: u16) -> Option<TerminalSize> {
        let before = self.viewport_size();
        self.emu.resize(cols, Self::viewport_rows(rows));
        let after = self.viewport_size();
        (after != before).then_some(after)
    }

    /// Render the terminal inside `rect`. `focused` marks it as the pane
    /// owning the keyboard.
    pub fn draw_in(&self, f: &mut Frame, rect: Rect, focused: bool) {
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

        let sig = self.signal();
        let name = if self.name.is_empty() {
            self.sid.to_string()
        } else {
            self.name.clone()
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
        if let Some(end) = &self.ended {
            let msg = match end {
                End::Exited(code) => format!(
                    " — exited{}",
                    code.map(|c| format!(" (code {c})")).unwrap_or_default()
                ),
                End::Interrupted => " — interrupted (process gone, history kept)".to_string(),
                End::Gone { reason } => match reason {
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
        f.render_widget(Paragraph::new(Text::from(self.emu.render())), emu_rect);

        // End-of-session banner over the emulator.
        if let Some(end) = &self.ended {
            let msg = match end {
                End::Exited(code) => format!(
                    " session exited{} — press any key to step out ",
                    code.map(|c| format!(" (code {c})")).unwrap_or_default()
                ),
                End::Interrupted => {
                    " session interrupted — the session process is gone (host restarted); history preserved — press any key to step out "
                        .to_string()
                }
                End::Gone { reason } => match reason {
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
}

/// One session's terminal: the established stream plus the screen snapshot.
/// The runner for the attach / resume / launch view — `⌃q` detaches.
pub(crate) struct Session {
    writer: ConnWriter,
    events: mpsc::UnboundedReceiver<HostEvent>,
    session: SessionId,
    meta: Option<SessionMeta>,
    screen: Vec<u8>,
}

impl Session {
    pub(crate) fn new(
        writer: ConnWriter,
        events: mpsc::UnboundedReceiver<HostEvent>,
        session: SessionId,
        meta: Option<SessionMeta>,
        screen: Vec<u8>,
    ) -> Self {
        Self {
            writer,
            events,
            session,
            meta,
            screen,
        }
    }

    /// Run the terminal on an already-initialized terminal until `⌃q` or the
    /// stream ends.
    pub(crate) async fn run(self, terminal: &mut ratatui::DefaultTerminal) -> Result<(), Error> {
        let Session {
            mut writer,
            mut events,
            session,
            meta,
            screen,
        } = self;
        let size = terminal.size()?;
        let (cols, rows) = (size.width, size.height);
        let mut pane = Terminal::new(session, meta, cols, rows);
        // The daemon's snapshot of the current screen — no history replay.
        pane.emu.feed(&screen);
        // The PTY must match the emulator's actual viewport (minus the title bar).
        let size = pane.viewport_size();
        let _ = writer.resize(&pane.sid, size).await;
        let mut stream = EventStream::new();
        loop {
            terminal.draw(|f| pane.draw_in(f, f.area(), true))?;
            tokio::select! {
                maybe = stream.next() => match maybe {
                    Some(Ok(Event::Key(k))) => match pane.press(&k) {
                        Keypress::StepOut => break,
                        Keypress::Send(bytes) => {
                            let _ = writer.input(&pane.sid, bytes).await;
                        }
                        Keypress::Scroll(_) | Keypress::Ignore => {}
                    },
                    Some(Ok(Event::Paste(s))) => {
                        let bytes = pane.paste_bytes(&s);
                        let _ = writer.input(&pane.sid, bytes).await;
                    }
                    Some(Ok(Event::Resize(cols, rows))) => {
                        if let Some(size) = pane.set_viewport(cols, rows) {
                            let _ = writer.resize(&pane.sid, size).await;
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) | None => break,
                },
                msg = events.recv() => match msg {
                    Some(m) => pane.handle_event(m),
                    None => break,
                },
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use bao_core::{
        alert::AlertInput,
        sandbox::{SandboxKind, WorkingCopy},
        types::{Command, Hostname},
    };

    use super::*;

    fn meta(status: Status, idle_secs: u64) -> SessionMeta {
        let now = 1_000_000u64;
        SessionMeta {
            id: SessionId::from_str("abc12345").unwrap(),
            name: Some("fix-auth".to_string()),
            command: "pi".to_string(),
            args: Command::from_args(vec!["pi".to_string()]),
            cwd: "/tmp".into(),
            working_copy: WorkingCopy {
                kind: SandboxKind::Worktree,
                repo: None,
                branch: Some("bao-abc12345".into()),
                path: "/tmp/tree".into(),
            },
            workspace: None,
            created: now - 60_000,
            host: Hostname::parse("localhost").unwrap(),
            status,
            last_activity: now - idle_secs * 1000,
            last_output: "hi".into(),
            alert: AlertInput { status, idle_secs }.alert(),
            waiting_for_input: None,
            idle_secs,
            age_secs: 60,
        }
    }

    #[test]
    fn title_signal_follows_the_shared_status_language() {
        let sid = SessionId::from_str("abc12345").unwrap();
        let mut t = Terminal::new(sid.clone(), Some(meta(Status::Running, 5)), 80, 24);
        assert_eq!(t.signal().glyph, '●');
        t.handle_event(HostEvent::State {
            ts: 0,
            meta: meta(Status::Running, 200),
        });
        assert_eq!(t.signal().glyph, '…');
    }

    #[test]
    fn keypresses_decide_purely_without_a_connection() {
        let sid = SessionId::from_str("abc12345").unwrap();
        let mut t = Terminal::new(sid, Some(meta(Status::Running, 5)), 80, 24);
        assert_eq!(
            t.press(&KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty())),
            Keypress::Send(b"a".to_vec())
        );
        // The keymap's reserved key, decided by the same pure path.
        assert_eq!(
            t.press(&KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL)),
            Keypress::StepOut
        );
        // Scroll applies to local state; the caller does nothing.
        assert_eq!(
            t.press(&KeyEvent::new(KeyCode::PageUp, KeyModifiers::empty())),
            Keypress::Scroll(12)
        );
        assert_eq!(t.scroll, 12);
    }

    #[test]
    fn once_ended_any_key_steps_out() {
        let sid = SessionId::from_str("abc12345").unwrap();
        let mut t = Terminal::new(sid, Some(meta(Status::Exited(Some(0)), 0)), 80, 24);
        t.handle_event(HostEvent::State {
            ts: 0,
            meta: meta(Status::Exited(Some(0)), 0),
        });
        assert_eq!(
            t.press(&KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty())),
            Keypress::StepOut
        );
    }

    #[test]
    fn paste_encoding_follows_modes_the_harness_set() {
        let sid = SessionId::from_str("abc12345").unwrap();
        let mut t = Terminal::new(sid, Some(meta(Status::Running, 5)), 80, 24);
        assert_eq!(t.paste_bytes("hi"), b"hi".to_vec());
        // The harness enabled bracketed paste on its output; input honors it.
        t.emu.feed(b"\x1b[?2004h");
        assert!(t.paste_bytes("hi").starts_with(b"\x1b[200~"));
    }
}
