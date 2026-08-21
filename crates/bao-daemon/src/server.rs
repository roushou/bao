//! The host daemon: accepts client connections and dispatches typed RPCs to
//! the session manager, streaming live events to attached clients.

use std::sync::Arc;

use bao_core::{
    error::Error,
    protocol::{FromHost, PROTOCOL_VERSION, Reply, Request, Rpc, WireError},
    sandbox::SandboxKind,
    types::{Addr, Command, DaemonInfo, Hostname, SessionId, SessionMeta, Status, now_ms},
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
                let _ = Connection::accept(stream, m).await;
            });
        }
    });
    Ok((actual, handle))
}

/// One client connection: reads requests, replies through a single writer
/// task, and keeps the event-stream tasks for attached sessions. The read
/// half is a `FrameReader<Request>`; outgoing messages travel as typed
/// `FromHost` over a channel to a `FrameWriter<FromHost>` task — callers
/// never touch frames or JSON.
struct Connection {
    reader: FrameReader<tokio::net::tcp::OwnedReadHalf, Request>,
    out_tx: mpsc::UnboundedSender<FromHost>,
    subs: Vec<tokio::task::JoinHandle<()>>,
    manager: Arc<Manager>,
}

impl Connection {
    async fn accept(stream: TcpStream, manager: Arc<Manager>) -> Result<(), Error> {
        let (read, write) = stream.into_split();
        let (out_tx, mut out_rx) = mpsc::unbounded_channel::<FromHost>();
        let writer = tokio::spawn(async move {
            let mut writer = FrameWriter::new(write);
            while let Some(msg) = out_rx.recv().await {
                if writer.write(&msg).await.is_err() {
                    break;
                }
            }
        });

        let mut conn = Connection {
            reader: FrameReader::new(read),
            out_tx,
            subs: Vec::new(),
            manager,
        };
        conn.run().await?;

        drop(conn.out_tx);
        for s in conn.subs {
            s.abort();
        }
        let _ = writer.await;
        Ok(())
    }

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
            Rpc::List => {
                let sessions = self.manager.list().iter().map(|s| s.meta()).collect();
                self.reply(id, Reply::List { sessions });
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
                );
            }
            Rpc::Watch => {
                // One subscription for all sessions: current picture for
                // every session, then the state bus. Lagging behind the bus
                // tail re-syncs from the manager (latest wins).
                let mut bus_rx = self.manager.subscribe_state();
                self.reply(id, Reply::Ok);
                for s in self.manager.list() {
                    let m = s.meta();
                    self.push_state(&m);
                }
                let out = self.out_tx.clone();
                let manager = self.manager.clone();
                self.subs.push(tokio::spawn(async move {
                    loop {
                        match bus_rx.recv().await {
                            Ok(StateEvent::Snapshot(meta)) => {
                                if out.send(FromHost::State { ts: now_ms(), meta }).is_err() {
                                    return;
                                }
                            }
                            Ok(StateEvent::Gone { session, reason }) => {
                                if out.send(FromHost::Gone { session, reason }).is_err() {
                                    return;
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                                for s in manager.list() {
                                    let m = s.meta();
                                    if out
                                        .send(FromHost::State {
                                            ts: now_ms(),
                                            meta: m,
                                        })
                                        .is_err()
                                    {
                                        return;
                                    }
                                }
                            }
                            Err(_) => return,
                        }
                    }
                }));
            }
            Rpc::Launch(launch) => {
                let command = match launch.command {
                    Some(c) => c,
                    None => match Command::parse("pi") {
                        Ok(c) => c,
                        Err(e) => {
                            self.err(id, e);
                            return Ok(());
                        }
                    },
                };
                let cwd = match launch.dir {
                    Some(d) if d.is_dir() => d,
                    Some(d) => {
                        self.err(id, format!("directory does not exist: {}", d.display()));
                        return Ok(());
                    }
                    None => match std::env::current_dir() {
                        Ok(c) => c,
                        Err(e) => {
                            self.err(id, e);
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
                        );
                        self.subscribe(sess, 0);
                    }
                    Err(e) => self.err(id, e),
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
                            );
                            self.subscribe(sess, 0);
                        }
                        Err(e) => self.err(id, e),
                    }
                }
                Err(e) => self.err(id, e),
            },
            Rpc::Attach { session } => match self.resolve(session) {
                Ok(sess) => {
                    let meta = sess.meta();
                    // One consistent (seq, screen) pair: the snapshot reflects
                    // exactly the output up to `seq`, so replaying from `seq`
                    // delivers the rest with no loss or duplication.
                    let (seq, screen) = sess.attach_point();
                    self.reply(
                        id,
                        Reply::Attach {
                            session: meta,
                            seq,
                            screen: bao_core::types::WireBytes(screen),
                        },
                    );
                    // Live from the current tail — the screen snapshot above
                    // carries the state; no history replay.
                    self.subscribe(sess, seq);
                }
                Err(e) => self.err(id, e),
            },
            Rpc::Input { session, data } => match self.resolve(session) {
                Ok(sess) => {
                    if matches!(sess.status(), Status::Exited(_)) {
                        self.err(id, "session has exited");
                        return Ok(());
                    }
                    match sess.input(&data) {
                        Ok(()) => self.reply(id, Reply::Ok),
                        Err(e) => self.err(id, e),
                    }
                }
                Err(e) => self.err(id, e),
            },
            Rpc::Resize { session, size } => match self.resolve(session) {
                Ok(s) => match s.resize(size) {
                    Ok(()) => self.reply(id, Reply::Ok),
                    Err(e) => self.err(id, e),
                },
                Err(e) => self.err(id, e),
            },
            Rpc::Stop { session } => match self.resolve(session) {
                Ok(s) => match s.kill() {
                    Ok(()) => self.reply(id, Reply::Ok),
                    Err(e) => self.err(id, e),
                },
                Err(e) => self.err(id, e),
            },
            Rpc::Rename { session, name } => match self.resolve(session) {
                Ok(s) => {
                    s.rename(name);
                    self.reply(id, Reply::Ok);
                }
                Err(e) => self.err(id, e),
            },
            Rpc::Rm { session } => match self.manager.remove(session.as_str()) {
                Ok(()) => self.reply(id, Reply::Ok),
                Err(e) => self.err(id, e),
            },
        }
        Ok(())
    }

    fn resolve(&self, session: SessionId) -> Result<Arc<Session>, Error> {
        self.manager.resolve(session.as_str())
    }

    fn reply(&self, id: u32, reply: Reply) {
        let _ = self.out_tx.send(FromHost::Reply { id, reply });
    }

    fn push_state(&self, m: &SessionMeta) {
        let _ = self.out_tx.send(FromHost::State {
            ts: now_ms(),
            meta: m.clone(),
        });
    }

    fn err(&self, id: u32, error: impl Into<WireError>) {
        let _ = self.out_tx.send(FromHost::Err {
            id,
            error: error.into(),
        });
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
                if out.send(FromHost::from(&ev)).is_err() {
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
                                if out.send(FromHost::from(&ev)).is_err() {
                                    return;
                                }
                            }
                            Ok(_) => {}
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                                // We fell behind the broadcast buffer; the log has it all.
                                let (snap, l) = sess.snapshot_and_last(last);
                                for ev in snap {
                                    if out.send(FromHost::from(&ev)).is_err() {
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
                            .is_err()
                        {
                            return;
                        }
                    }
                    bus = bus_rx.recv() => {
                        match bus {
                            Ok(StateEvent::Gone { session, reason }) if session == sid => {
                                let _ = out.send(FromHost::Gone { session, reason });
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
                                    });
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
