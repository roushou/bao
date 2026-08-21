//! The [`Tui`]: the terminal frontend. Consumers run the TUI — the overview
//! is the default, attaching to one session is the targeted form. The surface
//! is an internal detail.

use std::io::IsTerminal;

use bao_client::Conn;
use bao_core::types::SessionId;
use bao_transport::Addr;
use ratatui::DefaultTerminal;

use crate::{error::Error, overview::Overview, terminal::Session};

/// The two full-screen surfaces the app switches between. Sizes differ
/// meaningfully (the overview carries two connections), so the enum is
/// boxed-by-variant rather than padded.
#[allow(clippy::large_enum_variant)]
enum Surface {
    Overview(Overview),
    Session(Session),
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
            surface: Surface::Session(Session::new(writer, events, session, Some(meta), screen)),
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
            Surface::Session(session) => session.run(&mut terminal).await,
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
