//! The overview: owns the shared state, the components, and the
//! connections; runs the event loop and routes [`Action`]s.

use std::time::Duration;

use bao_client::{Conn, ConnWriter, HostEvent};
use bao_core::{
    sandbox::SandboxSpec,
    types::{SessionId, TerminalSize},
};
use bao_transport::Addr;
use crossterm::event::{Event as CrosstermEvent, EventStream};
use futures_util::{StreamExt, future::Either};
use ratatui::{Frame, layout::Rect, widgets::Paragraph};
use tokio::sync::mpsc;

use crate::{
    action::Action,
    components::{Component, Components},
    error::Error,
    event::Event,
    keys,
    state::{Ctx, Focus, Group, PromptAction, Row, Toast, sort_rows},
    terminal::Terminal,
};

const WIDE_MIN_COLS: u16 = 110;

/// The overview application.
pub struct Overview {
    rows: Vec<Row>,
    host: String,
    focus: Focus,
    status_line: String,
    flash: Option<Toast>,
    should_quit: bool,
    components: Components,
    writer: ConnWriter,
    events: mpsc::UnboundedReceiver<HostEvent>,
    twriter: Option<ConnWriter>,
    tevents: Option<mpsc::UnboundedReceiver<HostEvent>>,
    addr: Addr,
    reconnecting: bool,
    width: u16,
    height: u16,
}

impl Overview {
    pub(crate) fn new(
        host: String,
        addr: Addr,
        writer: ConnWriter,
        events: mpsc::UnboundedReceiver<HostEvent>,
    ) -> Self {
        Overview {
            rows: Vec::new(),
            host,
            focus: Focus::Rail,
            status_line: String::new(),
            flash: None,
            should_quit: false,
            components: Components::new(),
            writer,
            events,
            twriter: None,
            tevents: None,
            addr,
            reconnecting: false,
            width: 0,
            height: 0,
        }
    }

    pub(crate) async fn run_loop(
        &mut self,
        terminal: &mut ratatui::DefaultTerminal,
    ) -> Result<(), Error> {
        let _ = self.writer.watch().await;
        let _ = self.refresh().await;
        let size = terminal.size()?;
        self.width = size.width;
        self.height = size.height;
        if let Some(sid) = self.components.rail.selection().cloned() {
            self.ensure_terminal(&sid).await;
        }

        let mut stream = EventStream::new();
        loop {
            let size = terminal.size()?;
            self.width = size.width;
            self.height = size.height;
            terminal.draw(|f| self.render(f, size.width, size.height))?;

            enum Tick {
                Key(Option<std::result::Result<CrosstermEvent, std::io::Error>>),
                Host(HostEvent),
                Term(Option<HostEvent>),
                Refresh,
            }
            let terminal_fut = match self.tevents.as_mut() {
                Some(e) => Either::Left(e.recv()),
                None => Either::Right(futures_util::future::pending()),
            };
            let state_ev = async {
                if self.reconnecting {
                    futures_util::future::pending::<Option<HostEvent>>().await
                } else {
                    self.events.recv().await
                }
            };
            let tick = tokio::select! {
                ev = stream.next() => Tick::Key(ev),
                msg = state_ev => Tick::Host(msg.unwrap_or(HostEvent::Disconnected)),
                msg = terminal_fut => Tick::Term(msg),
                _ = tokio::time::sleep(Duration::from_secs(1)) => Tick::Refresh,
            };

            match tick {
                Tick::Key(Some(Ok(CrosstermEvent::Key(key)))) => {
                    self.dispatch_key(key).await;
                    if self.should_quit {
                        break;
                    }
                }
                Tick::Key(Some(Ok(CrosstermEvent::Resize(_, _)))) => {
                    self.resize_terminal().await;
                }
                Tick::Key(Some(Ok(CrosstermEvent::Paste(text)))) => {
                    if self.focus == Focus::Terminal {
                        self.terminal_paste(&text).await;
                    }
                }
                Tick::Key(Some(Ok(_))) => {}
                Tick::Key(Some(Err(_))) | Tick::Key(None) => break,
                Tick::Host(msg) => self.handle_host(msg).await,
                Tick::Term(msg) => self.handle_terminal(msg).await,
                Tick::Refresh => self.on_tick().await,
            }
        }
        Ok(())
    }

    async fn dispatch_key(&mut self, key: crossterm::event::KeyEvent) {
        let event = Event::Key(key);
        let tabs = self.tab_views();
        // Routing order is the panes contract: modal overlays, then text
        // entry, then the keymap for whatever scope owns the keyboard.
        let action = {
            let ctx = make_ctx(
                &self.rows,
                &self.host,
                self.focus,
                self.components.rail.selection().cloned(),
                &tabs,
                self.components.rail.filter().to_string(),
                self.components.rail.filtering(),
                self.status_line.clone(),
                self.flash
                    .as_ref()
                    .filter(|t| t.alive(4))
                    .map(|t| t.text.clone()),
                self.components.help.is_open(),
                self.components.palette.is_open(),
            );
            if self.components.help.is_open() {
                self.components.help.handle_events(Some(&event), &ctx)
            } else if self.components.palette.is_open() {
                self.components.palette.handle_events(Some(&event), &ctx)
            } else if self.components.footer.has_prompt() || self.components.footer.has_confirm() {
                self.components.footer.handle_events(Some(&event), &ctx)
            } else if self.components.rail.filtering() {
                self.components.rail.handle_events(Some(&event), &ctx)
            } else if self.focus == Focus::Terminal {
                // Raw passthrough is not a cross-component intent: the
                // terminal decides purely, effects are applied below.
                self.terminal_key(&key).await;
                Action::Noop
            } else {
                match keys::Keymap::defaults().resolve(keys::Scope::Rail, &key) {
                    Some(action) => action,
                    None => Action::Noop,
                }
            }
        };
        self.apply(action).await;

        // Selection drives the terminal_pane.
        if let Some(sid) = self.components.rail.selection().cloned() {
            self.ensure_terminal(&sid).await;
        }
    }

    async fn apply(&mut self, action: Action) {
        match action {
            Action::Noop => {}
            Action::Quit => self.should_quit = true,
            Action::MoveUp
            | Action::MoveDown
            | Action::PageUp
            | Action::PageDown
            | Action::First
            | Action::Last => {
                let a = action;
                self.components.rail.apply_cursor(&self.rows, &a);
            }
            Action::StartFilter => self.components.rail.start_filter(),
            Action::Open => {
                self.focus = Focus::Terminal;
                self.components.terminal_pane.set_fullscreen(true);
                if let Some(sid) = self.components.rail.selection().cloned() {
                    self.ensure_terminal(&sid).await;
                }
            }
            Action::FocusTerminal => {
                self.focus = Focus::Terminal;
                self.components.terminal_pane.set_fullscreen(false);
                if let Some(sid) = self.components.rail.selection().cloned() {
                    self.ensure_terminal(&sid).await;
                }
            }
            Action::StepOut => {
                self.focus = Focus::Rail;
                self.components.terminal_pane.set_fullscreen(false);
            }
            Action::Resume => self.resume_selected().await,
            Action::Stop => self.stop_selected().await,
            Action::Remove => {
                if self.components.rail.selection().is_some() {
                    self.components.footer.confirm_rm();
                }
            }
            Action::Rename => {
                if let Some(sid) = self.components.rail.selection().cloned() {
                    self.components
                        .footer
                        .start_prompt("rename to: ", PromptAction::Rename(sid));
                }
            }
            Action::Create => {
                self.components.footer.start_prompt(
                    "launch session name (enter for default): ",
                    PromptAction::Create,
                );
            }
            Action::OpenPalette => self.components.palette.open_palette(),
            Action::OpenHelp => self.components.help.open(),
            Action::PaletteConfirm => self.palette_confirm().await,
            Action::RenameSession(sid, name) => self.rename(&sid, name).await,
            Action::CreateSession(name) => self.create(name).await,
            Action::Rm(sid) => self.rm(&sid).await,
        }
    }

    fn render(&mut self, f: &mut Frame, cols: u16, rows: u16) {
        let tabs = self.tab_views();
        let ctx = make_ctx(
            &self.rows,
            &self.host,
            self.focus,
            self.components.rail.selection().cloned(),
            &tabs,
            self.components.rail.filter().to_string(),
            self.components.rail.filtering(),
            self.status_line.clone(),
            self.flash
                .as_ref()
                .filter(|t| t.alive(4))
                .map(|t| t.text.clone()),
            self.components.help.is_open(),
            self.components.palette.is_open(),
        );
        let l = layout(cols, rows);

        self.components.header.render(f, &ctx, l.header);
        f.render_widget(
            Paragraph::new(crate::components::rule(l.rule1.width)),
            l.rule1,
        );
        self.components.tabs.render(f, &ctx, l.tabs);

        if self.components.palette.is_open() {
            render_body(&mut self.components, f, &ctx, &l);
            let r = overlay(cols, rows, 72, 6, 18);
            self.components.palette.render(f, &ctx, r);
        } else if self.components.help.is_open() {
            render_body(&mut self.components, f, &ctx, &l);
            let r = overlay(cols, rows, 66, 8, 20);
            self.components.help.render(f, &ctx, r);
        } else if self.rows.is_empty() {
            f.render_widget(
                Paragraph::new(empty_lines(l.body.width, l.body.height)),
                l.body,
            );
        } else {
            render_body(&mut self.components, f, &ctx, &l);
        }

        f.render_widget(
            Paragraph::new(crate::components::rule(l.rule2.width)),
            l.rule2,
        );
        self.components.footer.render(f, &ctx, l.footer);
    }

    // -- effects -----------------------------------------------------------

    async fn refresh(&mut self) -> Result<(), Error> {
        let sessions = self.writer.list().await?;
        self.rows = sessions.iter().map(Row::from_meta).collect();
        sort_rows(&mut self.rows);
        self.components.rail.reconcile(&self.rows);
        Ok(())
    }

    async fn ensure_terminal(&mut self, sid: &SessionId) {
        let terminal_pane = terminal_pane_size((self.width, self.height));
        if self.components.terminal_pane.contains(sid) {
            if let Some(size) =
                self.components
                    .terminal_pane
                    .ensure_viewport(sid, terminal_pane.0, terminal_pane.1)
            {
                self.terminal_resize(sid, size).await;
            }
            self.components.terminal_pane.set_active(Some(sid.clone()));
            return;
        }
        if self.twriter.is_none() {
            match Conn::connect(&self.addr).await {
                Ok(conn) => {
                    let (w, e) = conn.into_parts();
                    self.twriter = Some(w);
                    self.tevents = Some(e);
                }
                Err(e) => {
                    self.status_line = format!("attach failed: {e:#}");
                    return;
                }
            }
        }
        let mut terminal = Terminal::new(sid.clone(), None, terminal_pane.0, terminal_pane.1);
        let size = terminal.viewport_size();
        if let Some(w) = self.twriter.as_mut() {
            let _ = w.resize(sid, size).await;
        }
        let meta = {
            let w = self.twriter.as_mut().expect("connection opened above");
            match w.attach(sid).await {
                Ok((session, _seq, screen)) => {
                    terminal.emu.feed(&screen);
                    session
                }
                Err(_) => {
                    self.status_line = "attach failed".to_string();
                    return;
                }
            }
        };
        terminal.set_meta(Some(meta));
        self.components.terminal_pane.insert(terminal);
        self.components.terminal_pane.set_active(Some(sid.clone()));
    }

    async fn resize_terminal(&mut self) {
        let Some(sid) = self.components.rail.selection().cloned() else {
            return;
        };
        let (sc, sr) = terminal_pane_size((self.width, self.height));
        if let Some(size) = self.components.terminal_pane.ensure_viewport(&sid, sc, sr) {
            self.terminal_resize(&sid, size).await;
        }
    }

    async fn terminal_resize(&mut self, sid: &SessionId, size: TerminalSize) {
        if let Some(w) = self.twriter.as_mut() {
            let _ = w.resize(sid, size).await;
        }
    }

    async fn terminal_key(&mut self, key: &crossterm::event::KeyEvent) {
        let Some((sid, keypress)) = self.components.terminal_pane.press(key) else {
            return;
        };
        match keypress {
            crate::terminal::Keypress::StepOut => {
                self.focus = Focus::Rail;
                self.components.terminal_pane.set_fullscreen(false);
            }
            crate::terminal::Keypress::Send(bytes) => {
                if let Some(w) = self.twriter.as_mut() {
                    let _ = w.input(&sid, bytes).await;
                }
            }
            // Scroll was already applied to local state by `press`.
            crate::terminal::Keypress::Scroll(_) | crate::terminal::Keypress::Ignore => {}
        }
    }

    /// Forward a paste to the active terminal — encoded per the harness's
    /// bracketed-paste mode.
    async fn terminal_paste(&mut self, text: &str) {
        let Some((sid, bytes)) = self.components.terminal_pane.paste_bytes(text) else {
            return;
        };
        if let Some(w) = self.twriter.as_mut() {
            let _ = w.input(&sid, bytes).await;
        }
    }

    async fn resume_selected(&mut self) {
        let Some(row) = self.selected_row() else {
            return;
        };
        if row.status != bao_core::types::Status::Interrupted {
            return;
        }
        let Some(sid) = self.components.rail.selection().cloned() else {
            return;
        };
        match self.writer.resume(&sid, TerminalSize::default()).await {
            Ok(_) => self.flash(format!("resumed {sid}")),
            Err(e) => self.status_line = format!("resume failed: {e:#}"),
        }
    }

    async fn stop_selected(&mut self) {
        let Some(row) = self.selected_row() else {
            return;
        };
        if row.status != bao_core::types::Status::Running {
            return;
        }
        let Some(sid) = self.components.rail.selection().cloned() else {
            return;
        };
        match self.writer.stop(&sid).await {
            Ok(()) => self.flash(format!("stopped {sid}")),
            Err(e) => self.status_line = format!("stop failed: {e:#}"),
        }
    }

    async fn rm(&mut self, sid: &SessionId) {
        match self.writer.rm(sid).await {
            Ok(()) => {
                self.components.terminal_pane.remove(sid);
                if self.components.rail.selection() == Some(sid) {
                    self.components.rail.set_selection(None);
                }
                self.flash(format!("removed {sid}"));
            }
            Err(e) => self.status_line = format!("rm failed: {e:#}"),
        }
    }

    async fn rename(&mut self, sid: &SessionId, name: Option<String>) {
        match self.writer.rename(sid, name.clone()).await {
            Ok(()) => {
                if let Some(n) = name {
                    self.flash(format!("renamed to {n}"));
                }
            }
            Err(e) => self.status_line = format!("rename failed: {e:#}"),
        }
    }

    async fn create(&mut self, name: Option<String>) {
        // Aim at the selected session's workspace when there is one — new
        // sessions land next to what you're already watching.
        let target = self.selected_row().and_then(|r| r.meta.workspace.clone());
        let result = match &target {
            Some(ws) => {
                self.writer
                    .launch_in(
                        ws,
                        None,
                        name,
                        TerminalSize::default(),
                        SandboxSpec::default(),
                    )
                    .await
            }
            None => {
                self.writer
                    .launch(
                        None,
                        None,
                        name,
                        TerminalSize::default(),
                        SandboxSpec::default(),
                    )
                    .await
            }
        };
        match result {
            Ok(session) => {
                // Select + dock the new session so its boot is visible in the
                // terminal pane while the the overview stays focused (the
                // backgrounded saga advances it preparing → starting → running).
                let sid = session.id;
                self.components.rail.set_selection(Some(sid.clone()));
                self.ensure_terminal(&sid).await;
            }
            Err(e) => self.status_line = format!("launch failed: {e:#}"),
        }
    }

    async fn palette_confirm(&mut self) {
        let selected = self
            .components
            .palette
            .entries(&self.rows)
            .get(self.components.palette.selected())
            .cloned();
        let Some(entry) = selected else {
            return;
        };
        match entry {
            crate::state::PaletteEntry::Session(id) => {
                self.components.rail.set_selection(Some(id.clone()));
                self.focus = Focus::Terminal;
                self.components.terminal_pane.set_fullscreen(true);
                self.ensure_terminal(&id).await;
            }
            crate::state::PaletteEntry::Create { name } => self.create(name).await,
            crate::state::PaletteEntry::Action { verb, id, .. } => {
                self.components.rail.set_selection(Some(id.clone()));
                match verb {
                    "resume" => {
                        let _ = self.resume_selected().await;
                    }
                    "stop" => {
                        let _ = self.stop_selected().await;
                    }
                    "remove" => self.rm(&id).await,
                    "rename" => {
                        self.components
                            .footer
                            .start_prompt("rename to: ", PromptAction::Rename(id));
                    }
                    _ => {}
                }
            }
        }
    }

    async fn handle_host(&mut self, msg: HostEvent) {
        match msg {
            HostEvent::State { meta, .. } => {
                let needs = |m: &bao_core::types::SessionMeta| {
                    Group::of(m.status, m.alert, m.waiting_for_input == Some(true))
                        == Group::NeedsYou
                };
                let was = self
                    .rows
                    .iter()
                    .find(|r| r.id == meta.id)
                    .map(|r| needs(&r.meta))
                    .unwrap_or(false);
                if !was && needs(&meta) {
                    let name = meta.name.clone().unwrap_or_else(|| meta.id.to_string());
                    self.components.header.flash(meta.id.clone());
                    self.flash(format!("▲ {name} needs alert"));
                }
                // Starting → Running: the boot-complete fact — a flash, never
                // an idle animation.
                let was_starting = self
                    .rows
                    .iter()
                    .find(|r| r.id == meta.id)
                    .map(|r| r.status == bao_core::types::Status::Starting)
                    .unwrap_or(false);
                if was_starting && meta.status == bao_core::types::Status::Running {
                    let name = meta.name.clone().unwrap_or_else(|| meta.id.to_string());
                    self.components.header.flash(meta.id.clone());
                    self.flash(format!("{name} started"));
                }
                if let Some(row) = self.rows.iter_mut().find(|r| r.id == meta.id) {
                    *row = Row::from_meta(&meta);
                }
                sort_rows(&mut self.rows);
            }
            HostEvent::Gone { session, reason } => {
                self.rows.retain(|r| r.id != session);
                self.components.terminal_pane.remove(&session);
                self.components.rail.reconcile(&self.rows);
                sort_rows(&mut self.rows);
                if let Some(reason) = reason {
                    self.flash(format!("launch failed: {reason}"));
                    self.status_line = format!("launch failed: {reason}");
                }
            }
            HostEvent::Disconnected => {
                self.status_line = "reconnecting…".to_string();
                self.components.terminal_pane.clear();
                self.tevents = None;
                self.reconnecting = true;
                if self.reconnect().await {
                    self.reconnecting = false;
                    self.status_line.clear();
                    self.flash("reconnected");
                }
            }
            _ => {}
        }
    }

    async fn handle_terminal(&mut self, msg: Option<HostEvent>) {
        match msg {
            Some(HostEvent::Disconnected) | None => {
                self.components.terminal_pane.clear();
                self.tevents = None;
                self.status_line = "terminal detached".to_string();
            }
            Some(ev) => {
                let sid = match &ev {
                    HostEvent::Output { session, .. } => Some(session.clone()),
                    HostEvent::Status { session, .. } => Some(session.clone()),
                    HostEvent::State { meta, .. } => Some(meta.id.clone()),
                    HostEvent::Gone { session, .. } => Some(session.clone()),
                    HostEvent::Disconnected => None,
                };
                if let Some(sid) = sid {
                    self.components.terminal_pane.handle_event(&sid, ev);
                }
            }
        }
    }

    async fn on_tick(&mut self) {
        if self.reconnecting {
            if self.reconnect().await {
                self.reconnecting = false;
                self.status_line.clear();
                self.flash("reconnected");
            }
        } else if self.refresh().await.is_err() {
            self.status_line = "reconnecting…".to_string();
            self.components.terminal_pane.clear();
            self.tevents = None;
            self.reconnecting = true;
        }
    }

    async fn reconnect(&mut self) -> bool {
        let Ok(conn) = Conn::connect(&self.addr).await else {
            return false;
        };
        self.host = conn.info().host.to_string();
        let (w, e) = conn.into_parts();
        let mut w = w;
        if w.watch().await.is_err() {
            return false;
        }
        self.writer = w;
        self.events = e;
        if self.refresh().await.is_err() {
            return false;
        }
        if let Some(sid) = self.components.rail.selection().cloned() {
            self.ensure_terminal(&sid).await;
        }
        true
    }

    fn selected_row(&self) -> Option<&Row> {
        self.components
            .rail
            .selection()
            .and_then(|id| self.rows.iter().find(|r| &r.id == id))
    }

    /// The tab bar's model: every open terminal, in open order, echoed as
    /// glyph + title. Pure derivation from the pane's order and the rows.
    fn tab_views(&self) -> Vec<crate::state::TabView> {
        self.components
            .terminal_pane
            .open()
            .iter()
            .map(|sid| {
                let row = self.rows.iter().find(|r| &r.id == sid);
                crate::state::TabView {
                    title: match row {
                        Some(r) if !r.name.is_empty() => r.name.clone(),
                        _ => sid.to_string(),
                    },
                    glyph: row.map(|r| r.glyph()).unwrap_or('·'),
                    style: row.map(|r| r.style()).unwrap_or_default(),
                    active: self.components.terminal_pane.active() == Some(sid),
                }
            })
            .collect()
    }

    fn flash(&mut self, msg: impl Into<String>) {
        self.flash = Some(Toast {
            text: msg.into(),
            at: std::time::Instant::now(),
        });
    }
}

// -- layout ----------------------------------------------------------------

struct Layout {
    header: Rect,
    rule1: Rect,
    tabs: Rect,
    body: Rect,
    rule2: Rect,
    footer: Rect,
    rail: Rect,
    divider: Rect,
    terminal_pane: Rect,
    wide: bool,
}

fn layout(cols: u16, rows: u16) -> Layout {
    let body_h = rows.saturating_sub(5);
    let wide = cols >= WIDE_MIN_COLS;
    let (rail, divider, terminal_pane) = if wide {
        let rail_w = (cols * 30 / 100).clamp(26, 40);
        (
            Rect::new(0, 3, rail_w, body_h),
            Rect::new(rail_w, 3, 1, body_h),
            Rect::new(rail_w + 1, 3, cols - rail_w - 1, body_h),
        )
    } else {
        let rail_h = (body_h / 3).clamp(4, 9);
        (
            Rect::new(0, 3, cols, rail_h),
            Rect::new(0, 3 + rail_h, 0, 0),
            Rect::new(0, 3 + rail_h, cols, body_h - rail_h),
        )
    };
    Layout {
        header: Rect::new(0, 0, cols, 1),
        rule1: Rect::new(0, 1, cols, 1),
        tabs: Rect::new(0, 2, cols, 1),
        body: Rect::new(0, 3, cols, body_h),
        rule2: Rect::new(0, rows - 2, cols, 1),
        footer: Rect::new(0, rows - 1, cols, 1),
        rail,
        divider,
        terminal_pane,
        wide,
    }
}

fn overlay(cols: u16, rows: u16, max_w: u16, min_h: u16, max_h: u16) -> Rect {
    let w = cols.min(max_w);
    let h = rows.saturating_sub(4).clamp(min_h, max_h);
    Rect::new((cols - w) / 2, 2, w, h)
}

fn terminal_pane_size((cols, rows): (u16, u16)) -> (u16, u16) {
    let body_h = rows.saturating_sub(5);
    if cols >= WIDE_MIN_COLS {
        let rail_w = (cols * 30 / 100).clamp(26, 40);
        (cols - rail_w - 1, body_h)
    } else {
        let rail_h = (body_h / 3).clamp(4, 9);
        (cols, body_h - rail_h)
    }
}

/// Draw the rail + divider + terminal terminal_pane.
fn render_body(components: &mut Components, f: &mut Frame, ctx: &Ctx, l: &Layout) {
    if components.terminal_pane.fullscreen() {
        components.terminal_pane.render(f, ctx, l.body);
        return;
    }
    components.rail.render(f, ctx, l.rail);
    if l.wide {
        f.render_widget(
            Paragraph::new(crate::components::vdivider(l.rail.height)),
            l.divider,
        );
    }
    components.terminal_pane.render(f, ctx, l.terminal_pane);
}

/// Build the shared read-only context, borrowing only the two fields that
/// stay live (rows + host) so components can be borrowed mutably alongside it.
#[allow(clippy::too_many_arguments)]
fn make_ctx<'a>(
    rows: &'a [Row],
    host: &'a str,
    focus: Focus,
    selection: Option<SessionId>,
    tabs: &'a [crate::state::TabView],
    filter: String,
    filtering: bool,
    status_line: String,
    toast: Option<String>,
    help_open: bool,
    palette_open: bool,
) -> Ctx<'a> {
    Ctx {
        rows,
        host,
        focus,
        selection,
        tabs,
        filter,
        filtering,
        status_line,
        toast,
        help_open,
        palette_open,
    }
}

fn empty_lines(w: u16, h: u16) -> Vec<ratatui::text::Line<'static>> {
    use ratatui::{
        style::{Color, Modifier, Style},
        text::{Line, Span},
    };
    let content: Vec<Line<'static>> = vec![
        Line::from(Span::styled(
            " Bao ",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "No sessions yet.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "c launch · ? help",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    let (w, h) = (w as usize, h as usize);
    let mut out = Vec::new();
    let top = h.saturating_sub(content.len() + 2) / 2;
    for _ in 0..top {
        out.push(Line::default());
    }
    for line in content {
        let text_w = line.width();
        let pad = w.saturating_sub(text_w) / 2;
        let mut spans = vec![Span::raw(" ".repeat(pad))];
        spans.extend(line.spans);
        out.push(Line::from(spans));
    }
    out
}
