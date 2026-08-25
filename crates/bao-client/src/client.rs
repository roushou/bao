//! The typed client: a connection to a bao daemon.
//!
//! This is the only surface frontends use. The wire vocabulary (`Rpc`,
//! `Reply`, `LaunchRequest`, `FromHost`, …) stays inside this crate; callers
//! speak typed methods and receive typed [`HostEvent`]s.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use bao_core::{
    registry::RegistryEntry,
    sandbox::SandboxSpec,
    types::{Command, Hostname, SessionId, SessionMeta, TerminalSize},
};
use bao_protocol::{
    ChannelKind, DaemonInfo, FromHost, LaunchRequest, PROTOCOL_VERSION, Reply, Request, Rpc,
    WireBytes, WireError,
};
use bao_transport::{
    Addr,
    frame::{FrameReader, FrameWriter},
};
use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot},
};

use crate::error::Error;

/// Route table for in-flight RPC replies (typed wire errors, so the caller
/// can branch on the kind).
type ReplyTable = Arc<Mutex<HashMap<u32, oneshot::Sender<std::result::Result<Reply, WireError>>>>>;

/// Anything the reader task wants the UI to know about. Payloads are moved,
/// never copied — the size spread is intentional.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum HostEvent {
    State {
        ts: u64,
        meta: SessionMeta,
    },
    Output {
        session: SessionId,
        seq: u64,
        data: Vec<u8>,
        ts: u64,
    },
    Status {
        session: SessionId,
        seq: u64,
        status: bao_core::types::Status,
        ts: u64,
    },
    Gone {
        session: SessionId,
        reason: Option<String>,
    },
    Disconnected,
}

/// A live connection: one-shot typed RPCs plus an event stream for attached
/// sessions. Split into [`ConnWriter`] + receiver when running the TUI so the
/// two halves can be borrowed independently in a `select!`.
pub struct Conn {
    writer: ConnWriter,
    events: mpsc::UnboundedReceiver<HostEvent>,
    /// What the daemon said about itself in the connect handshake.
    info: DaemonInfo,
}

/// The writer half of a connection: typed RPCs plus dedicated watch/attach
/// channels. Owns a clone of the event sender so channels opened after
/// [`Conn::into_parts`] still merge into the same event stream.
pub struct ConnWriter {
    writer: tokio::sync::Mutex<FrameWriter<tokio::net::tcp::OwnedWriteHalf, Request>>,
    replies: ReplyTable,
    next_id: u32,
    events_tx: mpsc::UnboundedSender<HostEvent>,
    addr: Addr,
}

/// A freshly opened channel: the handshake reply plus the two halves. The
/// write half is held by the channel's reader task so the server keeps the
/// channel alive — watch/attach are server-push, the client never writes
/// after the Hello.
struct Channel {
    reader: FrameReader<tokio::net::tcp::OwnedReadHalf, FromHost>,
    writer: FrameWriter<tokio::net::tcp::OwnedWriteHalf, FromHost>,
    reply: FromHost,
}

/// Convert a wire frame into a client event. `Reply`/`Err` are consumed by
/// the RPC route table, never surfaced as events.
fn host_event(f: FromHost) -> Option<HostEvent> {
    match f {
        FromHost::State { ts, meta } => Some(HostEvent::State { ts, meta }),
        FromHost::Output {
            session,
            seq,
            data,
            ts,
        } => Some(HostEvent::Output {
            session,
            seq,
            data: data.to_vec(),
            ts,
        }),
        FromHost::Status {
            session,
            seq,
            status,
            ts,
        } => Some(HostEvent::Status {
            session,
            seq,
            status,
            ts,
        }),
        FromHost::Gone { session, reason } => Some(HostEvent::Gone { session, reason }),
        FromHost::Reply { .. } | FromHost::Err { .. } => None,
    }
}

impl Conn {
    pub async fn connect(addr: &Addr) -> Result<Conn, Error> {
        let stream = match addr {
            Addr::Tcp { host, port } => {
                TcpStream::connect((*host, *port))
                    .await
                    .map_err(|source| Error::Unreachable {
                        addr: addr.clone(),
                        source,
                    })?
            }
            Addr::Unix(_) => return Err(Error::TransportUnsupported("unix socket")),
        };
        let (read, write) = stream.into_split();
        let (events_tx, events) = mpsc::unbounded_channel();
        let replies = Arc::new(Mutex::new(HashMap::<
            u32,
            oneshot::Sender<std::result::Result<Reply, WireError>>,
        >::new()));

        let r = replies.clone();
        let reader_events_tx = events_tx.clone();
        tokio::spawn(async move {
            let mut reader = FrameReader::<_, FromHost>::new(read);
            loop {
                match reader.read().await {
                    Ok(Some(FromHost::Reply { id, reply })) => {
                        if let Some(tx) = r.lock().unwrap().remove(&id) {
                            let _ = tx.send(Ok(reply));
                        }
                    }
                    Ok(Some(FromHost::Err { id, error })) => {
                        if let Some(tx) = r.lock().unwrap().remove(&id) {
                            let _ = tx.send(Err(error));
                        }
                    }
                    Ok(Some(f)) => {
                        if let Some(ev) = host_event(f) {
                            if reader_events_tx.send(ev).is_err() {
                                break;
                            }
                        }
                    }
                    Ok(None) => {
                        let _ = reader_events_tx.send(HostEvent::Disconnected);
                        break;
                    }
                    Err(_) => {
                        let _ = reader_events_tx.send(HostEvent::Disconnected);
                        break;
                    }
                }
            }
        });

        // Handshake: introduce the channel, learn who we're talking to, and
        // refuse to speak a different protocol version than our own.
        let mut conn = Conn {
            writer: ConnWriter {
                writer: tokio::sync::Mutex::new(FrameWriter::<_, Request>::new(write)),
                replies,
                next_id: 1,
                events_tx: events_tx.clone(),
                addr: addr.clone(),
            },
            events,
            info: DaemonInfo {
                host: Hostname::parse("unknown").expect("static hostname is valid"),
                protocol_version: 0,
                sandbox_backends: Vec::new(),
            },
        };
        match conn
            .writer
            .call(Rpc::Hello {
                kind: ChannelKind::Control,
            })
            .await?
        {
            Reply::Ok => {}
            _ => return Err(Error::UnexpectedReply),
        }
        let info = match conn.writer.call(Rpc::Info).await? {
            Reply::Info { info } => info,
            _ => return Err(Error::UnexpectedReply),
        };
        if info.protocol_version != PROTOCOL_VERSION {
            return Err(Error::VersionMismatch {
                server: info.protocol_version,
                client: PROTOCOL_VERSION,
            });
        }
        conn.info = info;
        Ok(conn)
    }

    /// What the daemon said about itself in the handshake: host, protocol
    /// version, and the isolation backends this machine can provide.
    pub fn info(&self) -> &DaemonInfo {
        &self.info
    }

    pub async fn list(&mut self) -> Result<Vec<SessionMeta>, Error> {
        self.writer.list().await
    }

    /// Subscribe to the daemon-wide state stream on its own channel: the
    /// daemon pushes every session's derived picture (current + changes). The
    /// overview — no byte streams, ever. Frames arrive on the shared event
    /// stream.
    pub async fn watch(&mut self) -> Result<(), Error> {
        self.writer.watch().await
    }

    /// Launch a session. Everything optional defers to the daemon:
    /// `command`/`profile` resolve host-side (explicit > profile > default),
    /// `dir` falls back to the daemon's cwd, `workspace` targets a
    /// registered root wherever you are.
    #[allow(clippy::too_many_arguments)]
    pub async fn launch(
        &mut self,
        command: Option<Command>,
        dir: Option<PathBuf>,
        workspace: Option<&str>,
        profile: Option<&str>,
        name: Option<String>,
        size: TerminalSize,
        sandbox: SandboxSpec,
    ) -> Result<SessionMeta, Error> {
        self.writer
            .launch(command, dir, workspace, profile, name, size, sandbox)
            .await
    }

    /// Attach to a session's terminal on its own channel: returns the
    /// consistent (seq, screen) snapshot; live bytes arrive on the shared
    /// event stream.
    pub async fn attach(
        &mut self,
        session: &SessionId,
    ) -> Result<(SessionMeta, u64, Vec<u8>), Error> {
        self.writer.attach(session).await
    }

    /// All registry entries on this host, sorted by alias.
    pub async fn registry_list(&mut self) -> Result<Vec<RegistryEntry>, Error> {
        self.writer.registry_list().await
    }

    /// Insert or replace a registry entry on this host (upsert by alias).
    pub async fn registry_put(&mut self, entry: RegistryEntry) -> Result<(), Error> {
        self.writer.registry_put(entry).await
    }

    /// Forget a registry entry by alias. Sessions already launched against
    /// it are untouched.
    pub async fn registry_remove(&mut self, alias: &str) -> Result<(), Error> {
        self.writer.registry_remove(alias).await
    }

    pub async fn input(
        &mut self,
        session: &SessionId,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<(), Error> {
        self.writer.input(session, bytes).await
    }

    pub async fn resume(
        &mut self,
        session: &SessionId,
        size: TerminalSize,
    ) -> Result<SessionMeta, Error> {
        self.writer.resume(session, size).await
    }

    pub async fn resize(&mut self, session: &SessionId, size: TerminalSize) -> Result<(), Error> {
        self.writer.resize(session, size).await
    }

    pub async fn stop(&mut self, session: &SessionId) -> Result<(), Error> {
        self.writer.stop(session).await
    }

    /// Rename a session (`None` clears its name).
    pub async fn rename(&mut self, session: &SessionId, name: Option<String>) -> Result<(), Error> {
        self.writer.rename(session, name).await
    }

    pub async fn rm(&mut self, session: &SessionId) -> Result<(), Error> {
        self.writer.rm(session).await
    }

    /// Split into the writer half and the event stream (for the TUI).
    pub fn into_parts(self) -> (ConnWriter, mpsc::UnboundedReceiver<HostEvent>) {
        (self.writer, self.events)
    }

    /// Next event pushed by the host (or disconnect notice).
    pub async fn next_event(&mut self) -> Option<HostEvent> {
        self.events.recv().await
    }
}

impl ConnWriter {
    /// Send a typed RPC and await the typed reply.
    async fn call(&mut self, rpc: Rpc) -> Result<Reply, Error> {
        let id = self.next_id;
        self.next_id += 1;
        let (tx, rx) = oneshot::channel();
        self.replies.lock().unwrap().insert(id, tx);
        {
            let mut w = self.writer.lock().await;
            w.write(&Request { id, rpc }).await?;
        }
        match rx.await {
            Ok(Ok(reply)) => Ok(reply),
            Ok(Err(e)) => Err(Error::Rpc(e)),
            Err(_) => Err(Error::LostConnection),
        }
    }

    pub async fn list(&mut self) -> Result<Vec<SessionMeta>, Error> {
        match self.call(Rpc::List).await? {
            Reply::List { sessions } => Ok(sessions),
            _ => Err(Error::UnexpectedReply),
        }
    }

    pub async fn watch(&mut self) -> Result<(), Error> {
        let chan = dial_channel(&self.addr, ChannelKind::Watch).await?;
        match chan.reply {
            FromHost::Reply {
                reply: Reply::Ok, ..
            } => {}
            FromHost::Err { error, .. } => return Err(Error::Rpc(error)),
            _ => return Err(Error::UnexpectedReply),
        }
        spawn_channel_reader(&self.events_tx, chan);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn launch(
        &mut self,
        command: Option<Command>,
        dir: Option<PathBuf>,
        workspace: Option<&str>,
        profile: Option<&str>,
        name: Option<String>,
        size: TerminalSize,
        sandbox: SandboxSpec,
    ) -> Result<SessionMeta, Error> {
        match self
            .call(Rpc::Launch(LaunchRequest {
                command,
                dir,
                workspace: workspace.map(String::from),
                profile: profile.map(String::from),
                name,
                size,
                sandbox,
            }))
            .await?
        {
            Reply::Launch { session } => Ok(session),
            _ => Err(Error::UnexpectedReply),
        }
    }

    pub async fn registry_list(&mut self) -> Result<Vec<RegistryEntry>, Error> {
        match self.call(Rpc::RegistryList).await? {
            Reply::Entries { entries } => Ok(entries),
            _ => Err(Error::UnexpectedReply),
        }
    }

    pub async fn registry_put(&mut self, entry: RegistryEntry) -> Result<(), Error> {
        match self.call(Rpc::RegistryPut { entry }).await? {
            Reply::Ok => Ok(()),
            _ => Err(Error::UnexpectedReply),
        }
    }

    pub async fn registry_remove(&mut self, alias: &str) -> Result<(), Error> {
        match self
            .call(Rpc::RegistryRemove {
                alias: alias.to_string(),
            })
            .await?
        {
            Reply::Ok => Ok(()),
            _ => Err(Error::UnexpectedReply),
        }
    }

    /// Attach to a session's terminal on its own channel: returns the
    /// consistent (seq, screen) snapshot; live bytes arrive on the shared
    /// event stream.
    pub async fn attach(
        &mut self,
        session: &SessionId,
    ) -> Result<(SessionMeta, u64, Vec<u8>), Error> {
        let chan = dial_channel(
            &self.addr,
            ChannelKind::Attach {
                session: session.clone(),
            },
        )
        .await?;
        let payload = match &chan.reply {
            FromHost::Reply {
                reply:
                    Reply::Attach {
                        session,
                        seq,
                        screen,
                    },
                ..
            } => (session.clone(), *seq, screen.to_vec()),
            FromHost::Err { error, .. } => return Err(Error::Rpc(error.clone())),
            _ => return Err(Error::UnexpectedReply),
        };
        spawn_channel_reader(&self.events_tx, chan);
        Ok(payload)
    }

    pub async fn input(
        &mut self,
        session: &SessionId,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<(), Error> {
        match self
            .call(Rpc::Input {
                session: session.clone(),
                data: WireBytes::from(bytes.into()),
            })
            .await?
        {
            Reply::Ok => Ok(()),
            _ => Err(Error::UnexpectedReply),
        }
    }

    pub async fn resume(
        &mut self,
        session: &SessionId,
        size: TerminalSize,
    ) -> Result<SessionMeta, Error> {
        match self
            .call(Rpc::Resume {
                session: session.clone(),
                size,
            })
            .await?
        {
            Reply::Resume { session } => Ok(session),
            _ => Err(Error::UnexpectedReply),
        }
    }

    pub async fn resize(&mut self, session: &SessionId, size: TerminalSize) -> Result<(), Error> {
        match self
            .call(Rpc::Resize {
                session: session.clone(),
                size,
            })
            .await?
        {
            Reply::Ok => Ok(()),
            _ => Err(Error::UnexpectedReply),
        }
    }

    pub async fn stop(&mut self, session: &SessionId) -> Result<(), Error> {
        match self
            .call(Rpc::Stop {
                session: session.clone(),
            })
            .await?
        {
            Reply::Ok => Ok(()),
            _ => Err(Error::UnexpectedReply),
        }
    }

    /// Rename a session (`None` clears its name).
    pub async fn rename(&mut self, session: &SessionId, name: Option<String>) -> Result<(), Error> {
        match self
            .call(Rpc::Rename {
                session: session.clone(),
                name,
            })
            .await?
        {
            Reply::Ok => Ok(()),
            _ => Err(Error::UnexpectedReply),
        }
    }

    pub async fn rm(&mut self, session: &SessionId) -> Result<(), Error> {
        match self
            .call(Rpc::Rm {
                session: session.clone(),
            })
            .await?
        {
            Reply::Ok => Ok(()),
            _ => Err(Error::UnexpectedReply),
        }
    }
}

/// Open a dedicated channel to the daemon: dial the same address, name the
/// channel in the Hello handshake, and read the single reply.
async fn dial_channel(addr: &Addr, kind: ChannelKind) -> Result<Channel, Error> {
    let stream = match addr {
        Addr::Tcp { host, port } => {
            TcpStream::connect((*host, *port))
                .await
                .map_err(|source| Error::Unreachable {
                    addr: addr.clone(),
                    source,
                })?
        }
        Addr::Unix(_) => return Err(Error::TransportUnsupported("unix socket")),
    };
    let (read, write) = stream.into_split();
    // Hello: name the channel, then re-bind the write half for FromHost
    // frames (the client never writes again on push-only channels).
    let mut w = FrameWriter::<_, Request>::new(write);
    w.write(&Request {
        id: 0,
        rpc: Rpc::Hello { kind },
    })
    .await?;
    let write = w.into_inner();

    let mut reader = FrameReader::<_, FromHost>::new(read);
    let reply = reader.read().await?.ok_or(Error::LostConnection)?;
    Ok(Channel {
        reader,
        writer: FrameWriter::new(write),
        reply,
    })
}

/// Stream one channel's frames into the shared event stream. The write half
/// is held for the task's lifetime so the server keeps the channel open; EOF
/// or a send failure ends it (and closes the socket).
fn spawn_channel_reader(events_tx: &mpsc::UnboundedSender<HostEvent>, chan: Channel) {
    let events_tx = events_tx.clone();
    tokio::spawn(async move {
        // Held: the server sees EOF only when we actually drop it.
        let _keep_write_half_open = chan.writer;
        let mut reader = chan.reader;
        loop {
            match reader.read().await {
                Ok(Some(frame)) => {
                    if let Some(ev) = host_event(frame) {
                        if events_tx.send(ev).is_err() {
                            return;
                        }
                    }
                }
                Ok(None) => {
                    let _ = events_tx.send(HostEvent::Disconnected);
                    return;
                }
                Err(_) => {
                    let _ = events_tx.send(HostEvent::Disconnected);
                    return;
                }
            }
        }
    });
}
