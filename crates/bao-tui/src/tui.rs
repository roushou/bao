//! The [`Tui`]: the terminal frontend. Consumers run the TUI — the overview
//! is the default, attaching to one session is the targeted form. The surface
//! is an internal detail.

use std::io::IsTerminal;

use bao_client::{Conn, ConnWriter, HostEvent};
use bao_core::types::{SessionId, SessionMeta};
use bao_transport::Addr;
use crossterm::event::{Event, EventStream};
use futures_util::StreamExt;
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;

use crate::{
    error::Error,
    overview::Overview,
    terminal::{Keypress, Terminal},
};

/// The two full-screen surfaces the app switches between. Sizes differ
/// meaningfully (the overview carries two connections), so the enum is
/// boxed-by-variant rather than padded.
#[allow(clippy::large_enum_variant)]
enum Surface {
    Overview(Overview),
    Attached(Attached),
}

pub struct Tui {
    terminal: DefaultTerminal,
    surface: Surface,
}

impl Tui {
    /// Run the TUI — the overview. Connects to `addr` and starts the watch
    /// stream itself.
    pub async fn run(addr: Addr) -> Result<(), Error> {
        ensure_tty()?;
        let conn = Conn::connect(&addr).await?;
        let host = conn.info().host.to_string();
        let (writer, events) = conn.into_parts();
        Self {
            terminal: ratatui::init(),
            surface: Surface::Overview(Overview::new(host, addr, writer, events)),
        }
        .run_surface()
        .await
    }

    /// Run the TUI attached to one session's terminal. Connects and attaches
    /// itself — for `bao attach`, and after `bao launch` / `bao resume`.
    pub async fn run_attached(addr: Addr, session: SessionId) -> Result<(), Error> {
        ensure_tty()?;
        let mut conn = Conn::connect(&addr).await?;
        let (meta, _, screen) = conn.attach(&session).await?;
        let (writer, events) = conn.into_parts();
        Self {
            terminal: ratatui::init(),
            surface: Surface::Attached(Attached::new(writer, events, session, Some(meta), screen)),
        }
        .run_surface()
        .await
    }

    async fn run_surface(self) -> Result<(), Error> {
        let Self {
            mut terminal,
            surface,
        } = self;
        let res = match surface {
            Surface::Overview(mut overview) => overview.run_loop(&mut terminal).await,
            Surface::Attached(attached) => attached.run(&mut terminal).await,
        };
        ratatui::restore();
        res
    }
}

fn ensure_tty() -> Result<(), Error> {
    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        return Err(Error::NotATerminal);
    }
    Ok(())
}

/// The attached surface: one session's live terminal, full-screen — the
/// runner behind `bao attach` / `bao resume` / post-launch attach. `⌃q`
/// detaches. Pure decisions come from [`Terminal::press`]; this shell only
/// performs the sends.
pub(crate) struct Attached {
    writer: ConnWriter,
    events: mpsc::UnboundedReceiver<HostEvent>,
    session: SessionId,
    meta: Option<SessionMeta>,
    screen: Vec<u8>,
}

impl Attached {
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

    /// Run on an already-initialized terminal until `⌃q` or the stream ends.
    pub(crate) async fn run(self, terminal: &mut DefaultTerminal) -> Result<(), Error> {
        let Attached {
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
            terminal.draw(|f| {
                crate::components::terminal_pane::render_terminal(f, f.area(), true, &pane)
            })?;
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
