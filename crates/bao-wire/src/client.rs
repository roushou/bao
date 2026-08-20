//! The typed client: a connection to a bao daemon.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use bao_core::{
    protocol::{FromHost, PROTOCOL_VERSION, Reply, Request, Rpc, WireError},
    types::{Addr, DaemonInfo, Hostname, LaunchRequest, SessionId, SessionMeta, TerminalSize},
};
use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot},
};

use crate::{
    error::Error,
    frame::{FrameReader, FrameWriter},
};

/// Route table for in-flight RPC replies (typed wire errors, so the caller
/// can branch on the kind).
type ReplyTable = Arc<Mutex<HashMap<u32, oneshot::Sender<std::result::Result<Reply, WireError>>>>>;

/// Anything the reader task wants the UI to know about. Payloads are moved,
/// never copied — the size spread is intentional.
#[allow(clippy::large_enum_variant)]
pub enum HostMsg {
    Frame(FromHost),
    Disconnected,
}

/// A live connection: one-shot typed RPCs plus an event stream for attached
/// sessions. Split into [`ConnWriter`] + receiver when running the TUI so the
/// two halves can be borrowed independently in a `select!`.
pub struct Conn {
    writer: ConnWriter,
    events: mpsc::UnboundedReceiver<HostMsg>,
    /// What the daemon said about itself in the connect handshake.
    info: DaemonInfo,
}

pub struct ConnWriter {
    writer: tokio::sync::Mutex<FrameWriter<tokio::net::tcp::OwnedWriteHalf, Request>>,
    replies: ReplyTable,
    next_id: u32,
}

impl Conn {
    pub async fn connect(addr: &Addr) -> Result<Conn, Error> {
        let stream = TcpStream::connect((addr.host(), addr.port()))
            .await
            .map_err(|source| Error::Unreachable {
                addr: *addr,
                source,
            })?;
        let (read, write) = stream.into_split();
        let (events_tx, events) = mpsc::unbounded_channel();
        let replies = Arc::new(Mutex::new(HashMap::<
            u32,
            oneshot::Sender<std::result::Result<Reply, WireError>>,
        >::new()));

        let r = replies.clone();
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
                        if events_tx.send(HostMsg::Frame(f)).is_err() {
                            break;
                        }
                    }
                    Ok(None) => {
                        let _ = events_tx.send(HostMsg::Disconnected);
                        break;
                    }
                    Err(_) => {
                        let _ = events_tx.send(HostMsg::Disconnected);
                        break;
                    }
                }
            }
        });

        // Handshake: learn who we're talking to, and refuse to speak a
        // different protocol version than our own.
        let mut conn = Conn {
            writer: ConnWriter {
                writer: tokio::sync::Mutex::new(FrameWriter::<_, Request>::new(write)),
                replies,
                next_id: 1,
            },
            events,
            info: DaemonInfo {
                host: Hostname::local(),
                protocol_version: 0,
                isolation_backends: Vec::new(),
            },
        };
        let info = match conn.call(Rpc::Info).await? {
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

    /// Send a typed RPC and await the typed reply.
    pub async fn call(&mut self, rpc: Rpc) -> Result<Reply, Error> {
        self.writer.call(rpc).await
    }

    pub async fn list(&mut self) -> Result<Vec<SessionMeta>, Error> {
        match self.call(Rpc::List).await? {
            Reply::List { sessions } => Ok(sessions),
            _ => Err(Error::UnexpectedReply),
        }
    }

    /// Subscribe to the daemon-wide state stream: the daemon pushes every
    /// session's derived picture (current + changes) onto this connection's
    /// event stream. The overview — no byte streams, ever.
    pub async fn watch(&mut self) -> Result<(), Error> {
        match self.call(Rpc::Watch).await? {
            Reply::Ok => Ok(()),
            _ => Err(Error::UnexpectedReply),
        }
    }

    pub async fn launch(&mut self, request: LaunchRequest) -> Result<SessionMeta, Error> {
        match self.call(Rpc::Launch(request)).await? {
            Reply::Launch { session } => Ok(session),
            _ => Err(Error::UnexpectedReply),
        }
    }

    pub async fn attach(
        &mut self,
        session: &SessionId,
    ) -> Result<(SessionMeta, u64, Vec<u8>), Error> {
        match self
            .call(Rpc::Attach {
                session: session.clone(),
            })
            .await?
        {
            Reply::Attach {
                session,
                seq,
                screen,
            } => Ok((session, seq, screen.to_vec())),
            _ => Err(Error::UnexpectedReply),
        }
    }

    pub async fn input(
        &mut self,
        session: &SessionId,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<(), Error> {
        match self
            .call(Rpc::Input {
                session: session.clone(),
                data: bytes.into().into(),
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

    /// Split into the writer half and the event stream (for the TUI).
    pub fn into_parts(self) -> (ConnWriter, mpsc::UnboundedReceiver<HostMsg>) {
        (self.writer, self.events)
    }

    /// Next event pushed by the host (or disconnect notice).
    pub async fn next_event(&mut self) -> Option<HostMsg> {
        self.events.recv().await
    }
}

impl ConnWriter {
    /// Send a typed RPC and await the typed reply.
    pub async fn call(&mut self, rpc: Rpc) -> Result<Reply, Error> {
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
}
