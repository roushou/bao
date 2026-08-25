//! The host daemon: accepts client connections and dispatches typed RPCs to
//! the session manager, streaming live events to attached clients.

use std::sync::Arc;

use bao_core::types::{Command, SessionId, Status, now_ms};
use bao_protocol::{
    ChannelKind, DaemonInfo, FromHost, PROTOCOL_VERSION, Reply, Request, Rpc, WireBytes, WireError,
};
use bao_transport::{
    Addr,
    frame::{FrameReader, FrameWriter},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpListener,
    sync::mpsc,
};

use crate::{
    error::Error,
    harness::HarnessRegistry,
    session::{Manager, Session, StateEvent},
};

/// How often the daemon re-derives time-based facts (idle alert) and
/// publishes them. Time moves even when sessions don't produce events, so a
/// tick is what lets stateless views see "idle" appear without polling.
const STATE_TICK_SECS: u64 = 5;

pub async fn serve(
    addr: Addr,
    manager: Arc<Manager>,
) -> Result<(Addr, tokio::task::JoinHandle<()>), Error> {
    let listener = match addr {
        Addr::Tcp { host, port } => TcpListener::bind((host, port)).await?,
        Addr::Unix(_) => return Err(Error::TransportUnsupported("unix socket")),
    };
    let actual = Addr::local(listener.local_addr()?.port());

    // Idle ticker: re-derive and publish state for every session on a clock,
    // not only on events.
    {
        let m = manager.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(STATE_TICK_SECS));
            tick.tick().await; // skip the immediate first tick (already published)
            loop {
                tick.tick().await;
                for s in m.list() {
                    // Status hooks: ask the harness what the session is doing.
                    // Honest — the adapter returns None when it cannot tell,
                    // and the session only stores what it reports.
                    if s.status() == Status::Running {
                        let harness = HarnessRegistry::identify(&s.command);
                        let working_copy = s.working_copy();
                        s.set_waiting(harness.waiting_for_input(&working_copy));
                    }
                    s.publish_state();
                }
            }
        });
    }

    let mgr = manager.clone();
    let handle = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let m = mgr.clone();
            tokio::spawn(async move {
                let _ = accept(stream, m).await;
            });
        }
    });
    Ok((actual, handle))
}

/// Bounded capacity of a control channel's outbound queue: replies and
/// subscribed session streams share it. Every send is awaited, so a slow
/// reader backs up here and never buffers without bound.
const CONTROL_OUT_CAP: usize = 4096;

/// Accept one connection: read its channel handshake and serve only that
/// channel until the peer closes. Cancellation is the socket closing — no
/// per-channel bookkeeping on the daemon side.
///
/// Generic over the stream so tests can drive the full protocol in-process
/// over `tokio::io::duplex` pairs (no daemon binary), and so a later unix
/// transport plugs in as another `AsyncRead + AsyncWrite`.
async fn accept<S>(stream: S, manager: Arc<Manager>) -> Result<(), Error>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (read, write) = tokio::io::split(stream);
    let mut reader = FrameReader::<_, Request>::new(read);
    // A channel must introduce itself as the first frame. Anything else
    // is a protocol violation — drop the connection.
    let Some(Request {
        id,
        rpc: Rpc::Hello { kind },
    }) = reader.read().await.ok().flatten()
    else {
        eprintln!("bao daemon: connection without a Hello handshake");
        return Ok(());
    };
    let read = reader.into_inner();
    match kind {
        ChannelKind::Control => run_control(read, write, manager, id).await,
        ChannelKind::Watch => {
            run_watch(FrameReader::new(read), FrameWriter::new(write), manager, id).await
        }
        ChannelKind::Attach { session } => run_attach(read, write, manager, id, session).await,
    }
}

/// The RPC channel: a bounded outbound queue behind one writer task (the
/// channel carries replies plus subscribed session streams), served until the
/// peer closes. Awaited sends are the backpressure — a slow reader blocks its
/// own channel and nothing else.
async fn run_control<R, W>(
    read: R,
    write: W,
    manager: Arc<Manager>,
    hello_id: u32,
) -> Result<(), Error>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (out_tx, mut out_rx) = mpsc::channel::<FromHost>(CONTROL_OUT_CAP);
    let writer = tokio::spawn(async move {
        let mut writer = FrameWriter::new(write);
        while let Some(msg) = out_rx.recv().await {
            if writer.write(&msg).await.is_err() {
                break;
            }
        }
    });

    let mut ctrl = Connection {
        reader: FrameReader::new(read),
        out_tx,
        subs: Vec::new(),
        manager,
    };
    ctrl.reply(hello_id, Reply::Ok).await?;
    let _ = ctrl.run().await;

    drop(ctrl.out_tx);
    for s in ctrl.subs {
        s.abort();
    }
    let _ = writer.await;
    Ok(())
}

/// The watch channel: writes directly to the socket — no intermediate queue,
/// TCP backpressure is the flow control — pushing the current picture of
/// every session, then following the state bus until the peer closes.
/// Push-only: stray frames are ignored.
async fn run_watch<R, W>(
    mut reader: FrameReader<R, Request>,
    mut writer: FrameWriter<W, FromHost>,
    manager: Arc<Manager>,
    hello_id: u32,
) -> Result<(), Error>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    writer
        .write(&FromHost::Reply {
            id: hello_id,
            reply: Reply::Ok,
        })
        .await?;
    for s in manager.list() {
        let m = s.meta();
        writer
            .write(&FromHost::State {
                ts: now_ms(),
                meta: m,
            })
            .await?;
    }
    let mut bus_rx = manager.subscribe_state();
    loop {
        tokio::select! {
            ev = bus_rx.recv() => match ev {
                Ok(StateEvent::Snapshot(meta)) => {
                    if writer.write(&FromHost::State { ts: now_ms(), meta }).await.is_err() {
                        return Ok(());
                    }
                }
                Ok(StateEvent::Gone { session, reason }) => {
                    if writer.write(&FromHost::Gone { session, reason }).await.is_err() {
                        return Ok(());
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    // Missed the bus tail; the manager's list is the truth.
                    for s in manager.list() {
                        let m = s.meta();
                        if writer
                            .write(&FromHost::State { ts: now_ms(), meta: m })
                            .await
                            .is_err()
                        {
                            return Ok(());
                        }
                    }
                }
                Err(_) => return Ok(()),
            },
            req = reader.read() => match req {
                Ok(None) | Err(_) => return Ok(()),
                Ok(Some(_)) => {}
            },
        }
    }
}

/// The attach channel: reply with a consistent (seq, screen) snapshot, then
/// stream the session's live events directly to the socket until the peer
/// closes. An unresolvable session gets a typed error, not a silent close.
async fn run_attach<R, W>(
    read: R,
    write: W,
    manager: Arc<Manager>,
    hello_id: u32,
    session: SessionId,
) -> Result<(), Error>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let mut reader = FrameReader::new(read);
    let mut writer = FrameWriter::new(write);
    let sess = match manager.resolve(session.as_str()) {
        Ok(s) => s,
        Err(e) => {
            writer
                .write(&FromHost::Err {
                    id: hello_id,
                    error: e.into(),
                })
                .await?;
            return Ok(());
        }
    };
    let meta = sess.meta();
    // One consistent (seq, screen) pair: the snapshot reflects exactly the
    // output up to `seq`, so replaying from `seq` delivers the rest with no
    // loss or duplication.
    let (seq, screen) = sess.attach_point();
    writer
        .write(&FromHost::Reply {
            id: hello_id,
            reply: Reply::Attach {
                session: meta,
                seq,
                screen: WireBytes(screen),
            },
        })
        .await?;
    stream_session_to(&mut writer, Some(&mut reader), &sess, &manager, seq).await
}

/// Where the daemon pushes one session's streamed events. The attach channel
/// writes straight to the socket; a control channel's per-session task sends
/// into the connection's bounded out-queue. This is the *only* difference
/// between the two streaming paths.
trait StreamSink {
    async fn push(&mut self, msg: FromHost) -> Result<(), Error>;
}

impl<W: AsyncWrite + Unpin> StreamSink for FrameWriter<W, FromHost> {
    async fn push(&mut self, msg: FromHost) -> Result<(), Error> {
        // bao_transport::Error → Error::Transport via #[from].
        self.write(&msg).await?;
        Ok(())
    }
}

impl StreamSink for mpsc::Sender<FromHost> {
    async fn push(&mut self, msg: FromHost) -> Result<(), Error> {
        self.send(msg).await.map_err(|_| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "peer closed",
            ))
        })
    }
}

/// Stream one session: replay the log from `after`, push the current derived
/// picture, then follow the live broadcast, the session's state channel, and
/// the removal bus — writing to whatever sink the channel uses. One copy of
/// the state machine serves both the attach channel (`read` = its
/// close-detection arm) and every control channel's per-session task
/// (`read` = `None`, the arm never fires). Backpressure is the sink's: TCP
/// flow control for the socket, the bounded out-queue for a control channel.
///
/// Correct under races: we snapshot the log under its lock, then skip
/// broadcast entries we already replayed; a lagged broadcast re-syncs from
/// the log (which has it all).
async fn stream_session_to<S, R>(
    sink: &mut S,
    mut read: Option<&mut FrameReader<R, Request>>,
    sess: &Arc<Session>,
    manager: &Arc<Manager>,
    after: u64,
) -> Result<(), Error>
where
    S: StreamSink,
    R: AsyncRead + Unpin,
{
    let mut state_rx = sess.state_subscribe();
    let mut rx = sess.subscribe();
    let mut bus_rx = manager.subscribe_state();
    let sid = sess.id.clone();

    let (snapshot, mut last) = sess.snapshot_and_last(after);
    for ev in snapshot {
        sink.push(FromHost::from(&ev)).await?;
    }
    // Current derived picture, then live.
    let current = sess.meta();
    sink.push(FromHost::State {
        ts: now_ms(),
        meta: current,
    })
    .await?;

    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                Ok(ev) if ev.seq > last => {
                    last = ev.seq;
                    sink.push(FromHost::from(&ev)).await?;
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    // Fell behind the broadcast buffer; the log has it all.
                    let (snap, l) = sess.snapshot_and_last(last);
                    for ev in snap {
                        sink.push(FromHost::from(&ev)).await?;
                    }
                    last = l;
                }
                Err(_) => return Ok(()),
            },
            changed = state_rx.changed() => {
                if changed.is_err() {
                    return Ok(());
                }
                let meta = state_rx.borrow().clone();
                sink.push(FromHost::State { ts: now_ms(), meta }).await?;
            }
            bus = bus_rx.recv() => match bus {
                Ok(StateEvent::Gone { session, reason }) if session == sid => {
                    let _ = sink.push(FromHost::Gone { session, reason }).await;
                    return Ok(());
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    // Missed some bus events; if the session is gone,
                    // `resolve` will fail.
                    if manager.resolve(sid.as_str()).is_err() {
                        let _ = sink
                            .push(FromHost::Gone {
                                session: sid.clone(),
                                reason: None,
                            })
                            .await;
                        return Ok(());
                    }
                }
                Err(_) => return Ok(()),
            },
            close = close_detect(&mut read) => return close,
        }
    }
}

/// The attach channel's close detection: exit on EOF or error; stray frames
/// are noise — the client never sends on an attach channel — and, like the
/// original loop, are ignored (the arm simply stops firing). With no reader
/// (control subscriptions) the arm never fires at all.
async fn close_detect<R: AsyncRead + Unpin>(
    read: &mut Option<&mut FrameReader<R, Request>>,
) -> Result<(), Error> {
    match read {
        Some(r) => match r.read().await {
            Ok(None) | Err(_) => Ok(()),
            Ok(Some(_)) => std::future::pending::<Result<(), Error>>().await,
        },
        None => std::future::pending::<Result<(), Error>>().await,
    }
}

/// One control connection: reads requests, replies through a single bounded
/// writer task, and keeps the event-stream tasks for launched/resumed
/// sessions. Outgoing messages travel as typed `FromHost` over a bounded
/// channel; awaited sends apply backpressure to a slow reader.
struct Connection<R>
where
    R: AsyncRead + Unpin,
{
    reader: FrameReader<R, Request>,
    out_tx: mpsc::Sender<FromHost>,
    subs: Vec<tokio::task::JoinHandle<()>>,
    manager: Arc<Manager>,
}

impl<R: AsyncRead + Unpin + Send> Connection<R> {
    async fn run(&mut self) -> Result<(), Error> {
        loop {
            let req = match self.reader.read().await {
                Ok(Some(r)) => r,
                Ok(None) => return Ok(()),
                Err(e) => {
                    // Garbage or a broken connection: drop it.
                    eprintln!("bao daemon: bad frame: {e}");
                    return Ok(());
                }
            };
            self.handle(req).await?;
        }
    }

    async fn handle(&mut self, req: Request) -> Result<(), Error> {
        let id = req.id;
        match req.rpc {
            Rpc::Hello { .. } => {
                // The channel introduced itself on accept; a second Hello is
                // noise, not a new channel.
                self.err(id, "hello already sent").await?;
            }
            Rpc::List => {
                let sessions = self.manager.list().iter().map(|s| s.meta()).collect();
                self.reply(id, Reply::List { sessions }).await?;
            }
            Rpc::Info => {
                // The machine's self-description: every client handshakes on
                // this before anything else.
                self.reply(
                    id,
                    Reply::Info {
                        info: DaemonInfo {
                            host: crate::hostname::resolve(),
                            protocol_version: PROTOCOL_VERSION,
                            sandbox_backends: crate::sandbox::available_backends(),
                        },
                    },
                )
                .await?;
            }
            Rpc::Launch(launch) => {
                let command = match launch.command {
                    Some(c) => c,
                    None => match Command::parse("pi") {
                        Ok(c) => c,
                        Err(e) => {
                            self.err(id, e).await?;
                            return Ok(());
                        }
                    },
                };
                // A targeted launch names its workspace; the daemon resolves
                // the alias against this host's registry. Alias wins over a
                // raw dir — targeting is the intent, the path is a detail.
                let dir = match &launch.workspace {
                    Some(ws) => {
                        let root = self
                            .manager
                            .workspaces()
                            .resolve(ws)
                            .map(|w| w.root.clone());
                        match root {
                            Some(d) => Some(d),
                            None => {
                                self.err(id, Error::UnknownWorkspace(ws.clone())).await?;
                                return Ok(());
                            }
                        }
                    }
                    None => launch.dir,
                };
                let cwd = match dir {
                    Some(d) if d.is_dir() => d,
                    Some(d) => {
                        self.err(id, format!("directory does not exist: {}", d.display()))
                            .await?;
                        return Ok(());
                    }
                    None => match std::env::current_dir() {
                        Ok(c) => c,
                        Err(e) => {
                            self.err(id, e).await?;
                            return Ok(());
                        }
                    },
                };
                match self
                    .manager
                    .launch(command, cwd, launch.size, launch.name, launch.sandbox)
                    .await
                {
                    Ok(sess) => {
                        self.reply(
                            id,
                            Reply::Launch {
                                session: sess.meta(),
                            },
                        )
                        .await?;
                        self.subscribe(sess, 0);
                    }
                    Err(e) => self.err(id, e).await?,
                }
            }
            Rpc::Resume { session, size } => match self.resolve(session) {
                Ok(sess) => {
                    let mut command = sess.command.clone();
                    if command.is_empty() {
                        command = Command::parse("pi").unwrap_or_default();
                    }
                    let working_copy = sess.working_copy();
                    if let Some(extra) =
                        HarnessRegistry::identify(&sess.command).resume_args(&working_copy)
                    {
                        command = Command::from_args(
                            command.as_args().iter().cloned().chain(extra).collect(),
                        );
                    }
                    match sess.resume(&command, size) {
                        Ok(()) => {
                            self.reply(
                                id,
                                Reply::Resume {
                                    session: sess.meta(),
                                },
                            )
                            .await?;
                            self.subscribe(sess, 0);
                        }
                        Err(e) => self.err(id, e).await?,
                    }
                }
                Err(e) => self.err(id, e).await?,
            },
            Rpc::Input { session, data } => match self.resolve(session) {
                Ok(sess) => {
                    if matches!(sess.status(), Status::Exited(_)) {
                        self.err(id, "session has exited").await?;
                        return Ok(());
                    }
                    match sess.input(&data) {
                        Ok(()) => self.reply(id, Reply::Ok).await?,
                        Err(e) => self.err(id, e).await?,
                    }
                }
                Err(e) => self.err(id, e).await?,
            },
            Rpc::Resize { session, size } => match self.resolve(session) {
                Ok(s) => match s.resize(size) {
                    Ok(()) => self.reply(id, Reply::Ok).await?,
                    Err(e) => self.err(id, e).await?,
                },
                Err(e) => self.err(id, e).await?,
            },
            Rpc::Stop { session } => match self.resolve(session) {
                Ok(s) => match s.kill() {
                    Ok(()) => self.reply(id, Reply::Ok).await?,
                    Err(e) => self.err(id, e).await?,
                },
                Err(e) => self.err(id, e).await?,
            },
            Rpc::Rename { session, name } => match self.resolve(session) {
                Ok(s) => {
                    s.rename(name);
                    self.reply(id, Reply::Ok).await?;
                }
                Err(e) => self.err(id, e).await?,
            },
            Rpc::Rm { session } => match self.manager.remove(session.as_str()) {
                Ok(()) => self.reply(id, Reply::Ok).await?,
                Err(e) => self.err(id, e).await?,
            },
            Rpc::WorkspaceList => {
                let workspaces = self
                    .manager
                    .workspaces()
                    .list()
                    .into_iter()
                    .cloned()
                    .collect();
                self.reply(id, Reply::Workspaces { workspaces }).await?;
            }
            Rpc::WorkspaceAdd { alias, path } => {
                let result = self.manager.workspaces_mut().add(&alias, &path);
                match result {
                    Ok(workspace) => self.reply(id, Reply::Workspace { workspace }).await?,
                    Err(e) => self.err(id, e).await?,
                }
            }
            Rpc::WorkspaceRemove { alias } => {
                let result = self.manager.workspaces_mut().remove(&alias);
                match result {
                    Ok(()) => self.reply(id, Reply::Ok).await?,
                    Err(e) => self.err(id, e).await?,
                }
            }
        }
        Ok(())
    }

    fn resolve(&self, session: SessionId) -> Result<Arc<Session>, Error> {
        self.manager.resolve(session.as_str())
    }

    async fn reply(&self, id: u32, reply: Reply) -> Result<(), Error> {
        self.out_tx
            .send(FromHost::Reply { id, reply })
            .await
            .map_err(|_| {
                Error::Io(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "peer closed",
                ))
            })
    }

    async fn err(&self, id: u32, error: impl Into<WireError>) -> Result<(), Error> {
        self.out_tx
            .send(FromHost::Err {
                id,
                error: error.into(),
            })
            .await
            .map_err(|_| {
                Error::Io(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "peer closed",
                ))
            })
    }

    /// Stream events for a session to this connection: the same state
    /// machine the attach channel runs ([`stream_session_to`]) — replay the
    /// log from `after`, then follow the broadcast, the session's state
    /// channel, and the removal bus — with this connection's bounded
    /// out-queue as the sink. No close-detection arm: the control loop's own
    /// reader owns that.
    fn subscribe(&mut self, sess: Arc<Session>, after: u64) {
        let mut out = self.out_tx.clone();
        let manager = self.manager.clone();
        self.subs.push(tokio::spawn(async move {
            let _ = stream_session_to::<mpsc::Sender<FromHost>, R>(
                &mut out, None, &sess, &manager, after,
            )
            .await;
        }));
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::Arc,
        time::{Duration, Instant},
    };

    use bao_core::{
        sandbox::{SandboxKind, SandboxSpec},
        types::{Command, SessionId, TerminalSize},
    };
    use bao_protocol::{
        ChannelKind, FromHost, LaunchRequest, PROTOCOL_VERSION, Reply, Request, Rpc, WireError,
    };
    use tokio::io::{AsyncWriteExt, DuplexStream, duplex};

    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bao-server-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn launch_request(dir: &std::path::Path, command: &str) -> LaunchRequest {
        LaunchRequest {
            command: Some(Command::parse(command).unwrap()),
            dir: Some(dir.to_path_buf()),
            workspace: None,
            name: None,
            size: TerminalSize::default(),
            sandbox: SandboxSpec {
                isolation: SandboxKind::InPlace,
            },
        }
    }

    /// A minimal in-process client: speaks the wire protocol over one end of
    /// a duplex pair — no daemon binary, no TCP. Mirrors the real client's
    /// reply routing: frames for other purposes (a launched session's event
    /// stream, watch state) are skipped until the RPC's own id answers.
    struct TestClient {
        reader: FrameReader<tokio::io::ReadHalf<DuplexStream>, FromHost>,
        writer: FrameWriter<tokio::io::WriteHalf<DuplexStream>, Request>,
        next_id: u32,
    }

    impl TestClient {
        fn new(stream: DuplexStream) -> Self {
            let (read, write) = tokio::io::split(stream);
            TestClient {
                reader: FrameReader::new(read),
                writer: FrameWriter::new(write),
                next_id: 1,
            }
        }

        /// One RPC round-trip: write the request, read frames until our id
        /// answers with `Reply` or `Err`.
        async fn call(&mut self, rpc: Rpc) -> Result<Reply, WireError> {
            let id = self.next_id;
            self.next_id += 1;
            self.writer
                .write(&Request { id, rpc })
                .await
                .map_err(|e| WireError::Internal {
                    message: e.to_string(),
                })?;
            loop {
                match self.reader.read().await.map_err(|e| WireError::Internal {
                    message: e.to_string(),
                })? {
                    Some(FromHost::Reply { id: rid, reply }) if rid == id => return Ok(reply),
                    Some(FromHost::Err { id: rid, error }) if rid == id => return Err(error),
                    Some(_) => continue,
                    None => {
                        return Err(WireError::Internal {
                            message: "connection closed before the reply".into(),
                        });
                    }
                }
            }
        }

        /// The next frame the daemon pushes on this channel.
        async fn next_frame(&mut self) -> Option<FromHost> {
            self.reader.read().await.ok().flatten()
        }
    }

    /// Spawn the daemon's accept loop over one end of a fresh duplex pair and
    /// return the client half plus the handshake reply. Every connection
    /// names its channel in the first frame.
    async fn dial(
        manager: &Arc<Manager>,
        kind: ChannelKind,
        buf: usize,
    ) -> (TestClient, Result<Reply, WireError>) {
        let (a, b) = duplex(buf);
        let m = manager.clone();
        tokio::spawn(async move {
            let _ = accept(a, m).await;
        });
        let mut client = TestClient::new(b);
        let reply = client.call(Rpc::Hello { kind }).await;
        (client, reply)
    }

    #[tokio::test]
    async fn control_channel_e2e_no_daemon_binary() {
        let root = temp_root("ctrl-e2e");
        let manager = Arc::new(Manager::new(root.clone(), root.join("working-copies")));
        let (mut control, reply) = dial(&manager, ChannelKind::Control, 64 * 1024).await;
        assert!(matches!(reply.unwrap(), Reply::Ok), "control handshake");

        // The daemon's self-description, with our protocol version.
        let Reply::Info { info } = control.call(Rpc::Info).await.unwrap() else {
            panic!("expected Info");
        };
        assert_eq!(info.protocol_version, PROTOCOL_VERSION);

        // Empty at first.
        let Reply::List { sessions } = control.call(Rpc::List).await.unwrap() else {
            panic!("expected List");
        };
        assert!(sessions.is_empty());

        // Launch a real process in place — no git, no daemon binary. The
        // reply carries the session's meta.
        let meta = match control
            .call(Rpc::Launch(launch_request(
                &root,
                "bash -c 'echo CHANNEL_E2E; sleep 5'",
            )))
            .await
            .unwrap()
        {
            Reply::Launch { session } => session,
            _ => panic!("expected Launch"),
        };

        // The new session shows up in List.
        let Reply::List { sessions } = control.call(Rpc::List).await.unwrap() else {
            panic!("expected List");
        };
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, meta.id);

        // Typed error: an unknown session is `NotFound`, not a generic
        // failure — clients branch on the kind.
        let err = control
            .call(Rpc::Stop {
                session: SessionId::generate(),
            })
            .await
            .unwrap_err();
        assert!(matches!(err, WireError::NotFound { .. }));

        // Rm brings the list back to empty (and kills the process).
        assert!(matches!(
            control.call(Rpc::Rm { session: meta.id }).await.unwrap(),
            Reply::Ok
        ));
        let Reply::List { sessions } = control.call(Rpc::List).await.unwrap() else {
            panic!("expected List");
        };
        assert!(sessions.is_empty());

        manager.kill_all();
    }

    #[tokio::test]
    async fn watch_channel_streams_state_and_gone() {
        let root = temp_root("watch-e2e");
        let manager = Arc::new(Manager::new(root.clone(), root.join("working-copies")));
        let (mut control, reply) = dial(&manager, ChannelKind::Control, 64 * 1024).await;
        assert!(matches!(reply.unwrap(), Reply::Ok));
        let (mut watch, reply) = dial(&manager, ChannelKind::Watch, 64 * 1024).await;
        assert!(matches!(reply.unwrap(), Reply::Ok));

        // The watch channel's initial picture is the (empty) list; the
        // launched session must then appear as a State frame, unpolled.
        let meta = match control
            .call(Rpc::Launch(launch_request(&root, "bash -c 'sleep 5'")))
            .await
            .unwrap()
        {
            Reply::Launch { session } => session,
            _ => panic!("expected Launch"),
        };

        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut seen = false;
        while tokio::time::Instant::now() < deadline {
            let frame = tokio::time::timeout(Duration::from_millis(500), watch.next_frame())
                .await
                .expect("watch stream must not stall")
                .expect("watch channel closed early");
            if let FromHost::State { meta: m, .. } = frame {
                if m.id == meta.id {
                    seen = true;
                    break;
                }
            }
        }
        assert!(seen, "launched session never appeared on the watch channel");

        // Rm → the watch channel learns the removal as Gone.
        assert!(matches!(
            control
                .call(Rpc::Rm {
                    session: meta.id.clone()
                })
                .await
                .unwrap(),
            Reply::Ok
        ));
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut gone = false;
        while tokio::time::Instant::now() < deadline {
            let frame = tokio::time::timeout(Duration::from_millis(500), watch.next_frame())
                .await
                .expect("watch stream must not stall")
                .expect("watch channel closed early");
            if let FromHost::Gone { session, .. } = frame {
                assert_eq!(session, meta.id);
                gone = true;
                break;
            }
        }
        assert!(gone, "removal never reached the watch channel");

        manager.kill_all();
    }

    #[tokio::test]
    async fn attach_channel_streams_snapshot_and_live_output() {
        let root = temp_root("attach-e2e");
        let manager = Arc::new(Manager::new(root.clone(), root.join("working-copies")));
        let (mut control, reply) = dial(&manager, ChannelKind::Control, 64 * 1024).await;
        assert!(matches!(reply.unwrap(), Reply::Ok));

        // A chatty process so the attach channel has live bytes to stream.
        let meta = match control
            .call(Rpc::Launch(launch_request(
                &root,
                "bash -c 'for i in 1 2 3 4 5; do echo ATTACH_$i; sleep 0.1; done'",
            )))
            .await
            .unwrap()
        {
            Reply::Launch { session } => session,
            _ => panic!("expected Launch"),
        };

        // Wait until the process has really printed, so the snapshot is
        // known to contain output — deterministic, no fixed sleep.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let Reply::List { sessions } = control.call(Rpc::List).await.unwrap() else {
                panic!("expected List");
            };
            if sessions
                .iter()
                .any(|m| m.id == meta.id && m.last_output.contains("ATTACH_1"))
            {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "session never produced ATTACH_1"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // The attach handshake reply IS the consistent snapshot:
        // (meta, seq, screen). Replay from `seq` continues without loss or
        // duplication — the snapshot is exactly the fold of events ≤ seq.
        let (mut attach, reply) = dial(
            &manager,
            ChannelKind::Attach {
                session: meta.id.clone(),
            },
            64 * 1024,
        )
        .await;
        let (seq, screen) = match reply.unwrap() {
            Reply::Attach {
                session,
                seq,
                screen,
            } => {
                assert_eq!(session.id, meta.id);
                (seq, screen.0)
            }
            other => panic!("expected Attach handshake, got {other:?}"),
        };
        // The screen is the terminal render of every event ≤ seq, so the
        // bytes we already saw are folded in — the re-attach contract.
        assert!(
            String::from_utf8_lossy(&screen).contains("ATTACH_1"),
            "snapshot must contain the output folded up to seq {seq}"
        );

        // Live frames: Output must arrive with seq strictly after the
        // snapshot's — nothing before it is re-sent (lossless, no dupes).
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut saw_output = false;
        while tokio::time::Instant::now() < deadline {
            let frame = tokio::time::timeout(Duration::from_millis(500), attach.next_frame())
                .await
                .expect("attach stream must not stall")
                .expect("attach channel closed early");
            if let FromHost::Output {
                session, seq: s, ..
            } = frame
            {
                assert_eq!(session, meta.id);
                assert!(s >= seq, "replay must not re-send seq ≤ snapshot");
                saw_output = true;
                break;
            }
        }
        assert!(saw_output, "no live output arrived on the attach channel");

        manager.kill_all();
    }

    /// A stalled reader on one channel must not slow any other channel: each
    /// channel owns its backpressure (bounded out queue + the transport's
    /// own flow control), so the daemon blocks that channel's writer and
    /// nothing else.
    #[tokio::test]
    async fn stalled_reader_does_not_block_other_channels() {
        let root = temp_root("isolation");
        let manager = Arc::new(Manager::new(root.clone(), root.join("working-copies")));

        // Channel A: control + a noisy session. After the launch ack we stop
        // reading entirely; with a 1 KiB duplex buffer the daemon's writer
        // for A backs up within a few hundred bytes of output.
        let (mut a, reply) = dial(&manager, ChannelKind::Control, 1024).await;
        assert!(matches!(reply.unwrap(), Reply::Ok));
        let meta = match a
            .call(Rpc::Launch(launch_request(
                &root,
                "bash -c 'while true; do echo STALL_ME; sleep 0.01; done'",
            )))
            .await
            .unwrap()
        {
            Reply::Launch { session } => session,
            _ => panic!("expected Launch"),
        };
        // Let A's writer fill the tiny duplex buffer and the bounded out
        // queue. `a` stays alive but unread — the stall is real.
        tokio::time::sleep(Duration::from_millis(1000)).await;

        // Channel B: a fresh control channel. RPCs must answer promptly even
        // while A is backed up end-to-end.
        let (mut b, reply) = dial(&manager, ChannelKind::Control, 64 * 1024).await;
        assert!(matches!(reply.unwrap(), Reply::Ok));
        for _ in 0..10 {
            let t0 = Instant::now();
            tokio::time::timeout(Duration::from_millis(500), b.call(Rpc::Info))
                .await
                .expect("channel B stalled behind channel A's backpressure")
                .unwrap();
            assert!(
                t0.elapsed() < Duration::from_millis(250),
                "B's round-trip must stay fast while A is stalled"
            );
        }

        // Cleanup: the noisy session is killed through its own RPC.
        assert!(matches!(
            b.call(Rpc::Rm { session: meta.id }).await.unwrap(),
            Reply::Ok
        ));
        manager.kill_all();
    }

    /// A connection whose first frame is not a channel Hello is a protocol
    /// violation — the daemon drops it without answering.
    #[tokio::test]
    async fn connection_without_hello_is_dropped() {
        let root = temp_root("refusal");
        let manager = Arc::new(Manager::new(root.clone(), root.join("working-copies")));
        let (a, b) = duplex(4096);
        let m = manager.clone();
        tokio::spawn(async move {
            let _ = accept(a, m).await;
        });
        let mut client = TestClient::new(b);

        // The first frame must name a channel; an RPC first is a refusal.
        client
            .writer
            .write(&Request {
                id: 1,
                rpc: Rpc::List,
            })
            .await
            .unwrap();
        let eof = tokio::time::timeout(Duration::from_secs(2), client.reader.read())
            .await
            .expect("daemon must close the connection")
            .unwrap();
        assert!(
            eof.is_none(),
            "a non-Hello first frame must not be answered"
        );
    }

    /// Garbage on the wire is refused the same way: dropped, not echoed.
    #[tokio::test]
    async fn garbage_first_frame_is_dropped() {
        let root = temp_root("refusal-garbage");
        let manager = Arc::new(Manager::new(root.clone(), root.join("working-copies")));
        let (a, mut raw) = duplex(4096);
        let m = manager.clone();
        tokio::spawn(async move {
            let _ = accept(a, m).await;
        });

        // Not a length-prefixed JSON frame at all.
        raw.write_all(b"not a frame, definitely not a hello")
            .await
            .unwrap();
        let mut client = TestClient::new(raw);
        let eof = tokio::time::timeout(Duration::from_secs(2), client.reader.read())
            .await
            .expect("daemon must close the connection")
            .unwrap();
        assert!(eof.is_none(), "garbage must be dropped, not echoed");
    }

    /// Attaching to a session the daemon doesn't know is a typed refusal —
    /// `NotFound` on the channel handshake, not a silent close.
    #[tokio::test]
    async fn attach_to_unknown_session_is_a_typed_refusal() {
        let root = temp_root("refusal-attach");
        let manager = Arc::new(Manager::new(root.clone(), root.join("working-copies")));
        let (_attach, reply) = dial(
            &manager,
            ChannelKind::Attach {
                session: SessionId::generate(),
            },
            4096,
        )
        .await;
        let err = reply.expect_err("attach to an unknown session must be refused");
        assert!(matches!(err, WireError::NotFound { .. }));
    }

    /// Workspaces: register → list → targeted launch lands in the
    /// workspace's root → unknown alias is a typed refusal → remove.
    #[tokio::test]
    async fn workspaces_register_target_and_remove() {
        let root = temp_root("workspaces");
        std::fs::create_dir_all(root.join("app")).unwrap();
        let manager = Arc::new(Manager::new(root.clone(), root.join("working-copies")));
        let (mut control, _) = dial(&manager, ChannelKind::Control, 64 * 1024).await;

        // Register.
        let Reply::Workspace { workspace } = control
            .call(Rpc::WorkspaceAdd {
                alias: "myapp".into(),
                path: root.join("app"),
            })
            .await
            .unwrap()
        else {
            panic!("expected Workspace");
        };
        assert_eq!(workspace.alias, "myapp");
        assert!(workspace.root.is_absolute());

        // List shows it.
        let Reply::Workspaces { workspaces } = control.call(Rpc::WorkspaceList).await.unwrap()
        else {
            panic!("expected Workspaces");
        };
        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].alias, "myapp");

        // A launch aimed at the workspace runs at its root — no dir sent.
        let launched = match control
            .call(Rpc::Launch(LaunchRequest {
                command: Some(Command::parse("bash -c 'sleep 5'").unwrap()),
                dir: None,
                workspace: Some("myapp".into()),
                name: None,
                size: TerminalSize::default(),
                sandbox: bao_core::sandbox::SandboxSpec {
                    isolation: bao_core::sandbox::SandboxKind::InPlace,
                },
            }))
            .await
            .unwrap()
        {
            Reply::Launch { session } => session,
            other => panic!("expected Launch, got {other:?}"),
        };
        assert!(launched.working_copy.path.starts_with(&workspace.root));

        // An unknown alias is a typed refusal, not a guess.
        let err = control
            .call(Rpc::Launch(LaunchRequest {
                command: Some(Command::parse("bash -c 'sleep 5'").unwrap()),
                dir: None,
                workspace: Some("nope".into()),
                name: None,
                size: TerminalSize::default(),
                sandbox: bao_core::sandbox::SandboxSpec {
                    isolation: bao_core::sandbox::SandboxKind::InPlace,
                },
            }))
            .await
            .expect_err("unknown workspace must refuse");
        assert!(matches!(err, WireError::UnknownWorkspace { .. }));

        // Remove; the alias stops resolving. The session itself is untouched.
        control
            .call(Rpc::WorkspaceRemove {
                alias: "myapp".into(),
            })
            .await
            .unwrap();
        let Reply::Workspaces { workspaces } = control.call(Rpc::WorkspaceList).await.unwrap()
        else {
            panic!("expected Workspaces");
        };
        assert!(workspaces.is_empty());
        manager.kill_all();
    }
}
