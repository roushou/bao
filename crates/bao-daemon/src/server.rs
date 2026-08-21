//! The host daemon: accepts client connections and dispatches typed RPCs to
//! the session manager, streaming live events to attached clients.

use std::sync::Arc;

use bao_core::{
    error::Error,
    protocol::{ChannelKind, FromHost, PROTOCOL_VERSION, Reply, Request, Rpc, WireError},
    sandbox::SandboxKind,
    types::{Addr, Command, DaemonInfo, Hostname, SessionId, Status, WireBytes, now_ms},
};
use bao_wire::frame::{FrameReader, FrameWriter};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
};

use crate::{
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
                        let workspace = s.workspace();
                        s.set_waiting(harness.waiting_for_input(&workspace));
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
async fn accept(stream: TcpStream, manager: Arc<Manager>) -> Result<(), Error> {
    let (read, write) = stream.into_split();
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
async fn run_control(
    read: tokio::net::tcp::OwnedReadHalf,
    write: tokio::net::tcp::OwnedWriteHalf,
    manager: Arc<Manager>,
    hello_id: u32,
) -> Result<(), Error> {
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
async fn run_watch(
    mut reader: FrameReader<tokio::net::tcp::OwnedReadHalf, Request>,
    mut writer: FrameWriter<tokio::net::tcp::OwnedWriteHalf, FromHost>,
    manager: Arc<Manager>,
    hello_id: u32,
) -> Result<(), Error> {
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
async fn run_attach(
    read: tokio::net::tcp::OwnedReadHalf,
    write: tokio::net::tcp::OwnedWriteHalf,
    manager: Arc<Manager>,
    hello_id: u32,
    session: SessionId,
) -> Result<(), Error> {
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
    stream_session(&mut writer, &mut reader, &sess, &manager, seq).await
}

/// Stream one session to an attached channel: replay the log from `after`,
/// then follow the live broadcast, the session's state channel, and the
/// removal bus — writing directly to the socket. Backpressure is TCP's; a
/// lagged broadcast re-syncs from the log.
async fn stream_session(
    writer: &mut FrameWriter<tokio::net::tcp::OwnedWriteHalf, FromHost>,
    reader: &mut FrameReader<tokio::net::tcp::OwnedReadHalf, Request>,
    sess: &Arc<Session>,
    manager: &Arc<Manager>,
    after: u64,
) -> Result<(), Error> {
    let mut state_rx = sess.state_subscribe();
    let mut rx = sess.subscribe();
    let mut bus_rx = manager.subscribe_state();
    let sid = sess.id.clone();

    let (snapshot, mut last) = sess.snapshot_and_last(after);
    for ev in snapshot {
        writer.write(&FromHost::from(&ev)).await?;
    }
    // Current derived picture, then live.
    let current = sess.meta();
    writer
        .write(&FromHost::State {
            ts: now_ms(),
            meta: current,
        })
        .await?;

    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                Ok(ev) if ev.seq > last => {
                    last = ev.seq;
                    writer.write(&FromHost::from(&ev)).await?;
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    // We fell behind the broadcast buffer; the log has it all.
                    let (snap, l) = sess.snapshot_and_last(last);
                    for ev in snap {
                        writer.write(&FromHost::from(&ev)).await?;
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
                writer.write(&FromHost::State { ts: now_ms(), meta }).await?;
            }
            bus = bus_rx.recv() => match bus {
                Ok(StateEvent::Gone { session, reason }) if session == sid => {
                    let _ = writer.write(&FromHost::Gone { session, reason }).await;
                    return Ok(());
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    // Missed some bus events; if the session is gone,
                    // `resolve` will fail.
                    if manager.resolve(sid.as_str()).is_err() {
                        let _ = writer
                            .write(&FromHost::Gone {
                                session: sid.clone(),
                                reason: None,
                            })
                            .await;
                        return Ok(());
                    }
                }
                Err(_) => return Ok(()),
            },
            req = reader.read() => match req {
                // The client never sends on an attach channel; exit on close.
                Ok(None) | Err(_) => return Ok(()),
                Ok(Some(_)) => {}
            },
        }
    }
}

/// One control connection: reads requests, replies through a single bounded
/// writer task, and keeps the event-stream tasks for launched/resumed
/// sessions. Outgoing messages travel as typed `FromHost` over a bounded
/// channel; awaited sends apply backpressure to a slow reader.
struct Connection {
    reader: FrameReader<tokio::net::tcp::OwnedReadHalf, Request>,
    out_tx: mpsc::Sender<FromHost>,
    subs: Vec<tokio::task::JoinHandle<()>>,
    manager: Arc<Manager>,
}

impl Connection {
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
                            host: Hostname::local(),
                            protocol_version: PROTOCOL_VERSION,
                            isolation_backends: vec![SandboxKind::InPlace, SandboxKind::Worktree],
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
                let cwd = match launch.dir {
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
                    let workspace = sess.workspace();
                    if let Some(extra) =
                        HarnessRegistry::identify(&sess.command).resume_args(&workspace)
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

    /// Stream events for a session to this connection: replay the log from
    /// `after`, then follow the live broadcast — and, in parallel, follow the
    /// session's state channel. Correct under races: we snapshot the log
    /// under its lock, then skip broadcast entries we already replayed.
    fn subscribe(&mut self, sess: Arc<Session>, after: u64) {
        let out = self.out_tx.clone();
        let manager = self.manager.clone();
        let sid = sess.id.clone();
        self.subs.push(tokio::spawn(async move {
            let mut state_rx = sess.state_subscribe();
            let mut rx = sess.subscribe();
            // A per-session stream also watches the state bus for this
            // session's removal, so an attached terminal learns a rolled-back
            // launch (or an `rm`) instead of sitting empty forever.
            let mut bus_rx = manager.subscribe_state();
            let (snapshot, last) = sess.snapshot_and_last(after);
            let mut last = last;
            for ev in snapshot {
                if out.send(FromHost::from(&ev)).await.is_err() {
                    return;
                }
            }
            // Current derived picture, then live.
            let current = sess.meta();
            if out
                .send(FromHost::State {
                    ts: now_ms(),
                    meta: current,
                })
                .await
                .is_err()
            {
                return;
            }
            loop {
                tokio::select! {
                    ev = rx.recv() => {
                        match ev {
                            Ok(ev) if ev.seq > last => {
                                last = ev.seq;
                                if out.send(FromHost::from(&ev)).await.is_err() {
                                    return;
                                }
                            }
                            Ok(_) => {}
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                                // We fell behind the broadcast buffer; the log has it all.
                                let (snap, l) = sess.snapshot_and_last(last);
                                for ev in snap {
                                    if out.send(FromHost::from(&ev)).await.is_err() {
                                        return;
                                    }
                                }
                                last = l;
                            }
                            Err(_) => return,
                        }
                    }
                    changed = state_rx.changed() => {
                        if changed.is_err() {
                            return;
                        }
                        let meta = state_rx.borrow().clone();
                        if out
                            .send(FromHost::State {
                                ts: now_ms(),
                                meta,
                            })
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                    bus = bus_rx.recv() => {
                        match bus {
                            Ok(StateEvent::Gone { session, reason }) if session == sid => {
                                let _ = out.send(FromHost::Gone { session, reason }).await;
                                return;
                            }
                            Ok(_) => {}
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                                // Missed some bus events; if the session is
                                // gone, `resolve` will fail.
                                if manager.resolve(sid.as_str()).is_err() {
                                    let _ = out.send(FromHost::Gone {
                                        session: sid.clone(),
                                        reason: None,
                                    }).await;
                                    return;
                                }
                            }
                            Err(_) => return,
                        }
                    }
                }
            }
        }));
    }
}
