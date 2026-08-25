//! The daemon lifecycle: build, start, graceful shutdown.
//!
//! A [`Daemon`] is the supervisor object: it owns the session [`Manager`],
//! the bound listener, and the server tasks (the accept loop and the idle
//! ticker). Construction goes through [`DaemonBuilder`] — the only way in —
//! and serving starts with [`Daemon::start`], which yields a
//! [`RunningDaemon`] whose [`RunningDaemon::shutdown`] is the graceful stop
//! (kill and flush sessions, join the server tasks).
//!
//! The process-level concerns — the exclusive PID lock, signal handling,
//! logging — stay with the binary that runs the daemon ([`DaemonBuilder`]
//! does not touch them), so a library consumer (a test, a future embedded
//! host) gets the server without the singleton semantics.

use std::{sync::Arc, time::Duration};

use bao_core::types::Status;
use bao_transport::Addr;
use tokio::{net::TcpListener, task::JoinHandle};

use crate::{error::Error, harness::HarnessRegistry, home::Home, server, session::Manager};

/// Default cadence for re-deriving time-based facts (idle alert) and
/// publishing them. Time moves even when sessions don't produce events, so
/// a tick is what lets stateless views see "idle" appear without polling.
const DEFAULT_TICK: Duration = Duration::from_secs(5);

/// A callback run once the daemon is serving, with the actual bound address
/// (which differs from the requested one when port 0 was used). This is the
/// signal a caller — or a test — waits on instead of polling for
/// connectivity. One-shot: the daemon starts once, so the hook is `FnOnce`.
pub type ReadyHook = dyn FnOnce(Addr) + Send + Sync;

/// Configuration for a [`Daemon`]. `home` and `addr` are required and go in
/// [`DaemonBuilder::new`]; everything else has a sensible default.
pub struct DaemonBuilder {
    home: Home,
    addr: Addr,
    tick: Duration,
    ready: Option<Box<ReadyHook>>,
}

impl DaemonBuilder {
    /// The home the daemon owns (sessions, registry, working copies) and
    /// the address to listen on.
    pub fn new(home: Home, addr: Addr) -> Self {
        Self {
            home,
            addr,
            tick: DEFAULT_TICK,
            ready: None,
        }
    }

    /// The home the daemon owns.
    pub fn home(mut self, home: Home) -> Self {
        self.home = home;
        self
    }

    /// The address to listen on; port 0 asks the OS for an ephemeral port
    /// (the actual address is reported by [`Daemon::addr`]).
    pub fn addr(mut self, addr: Addr) -> Self {
        self.addr = addr;
        self
    }

    /// How often the idle ticker re-derives and publishes state. Tests use
    /// a short tick; the default matches wall-clock second-level signals.
    pub fn tick(mut self, tick: Duration) -> Self {
        self.tick = tick;
        self
    }

    /// A hook invoked once the daemon is serving, with the actual bound
    /// address.
    pub fn on_ready(mut self, hook: impl FnOnce(Addr) + Send + Sync + 'static) -> Self {
        self.ready = Some(Box::new(hook));
        self
    }

    /// Open the manager (restoring the registry and any sessions on disk)
    /// and bind the listener. Both fail fast: a broken home or a taken
    /// port surfaces here, before any task is spawned.
    pub async fn build(self) -> Result<Daemon, Error> {
        std::fs::create_dir_all(self.home.root())?;
        let manager = Arc::new(Manager::open(&self.home)?);
        let listener = match self.addr {
            Addr::Tcp { host, port } => TcpListener::bind((host, port)).await?,
            Addr::Unix(_) => return Err(Error::TransportUnsupported("unix socket")),
        };
        let actual = Addr::local(listener.local_addr()?.port());
        Ok(Daemon {
            actual,
            listener,
            manager,
            tick: self.tick,
            ready: self.ready,
        })
    }
}

/// A built daemon: home opened and listener bound, nothing serving yet.
/// Call [`Daemon::start`] to serve; dropping without starting binds
/// nothing further.
pub struct Daemon {
    actual: Addr,
    listener: TcpListener,
    manager: Arc<Manager>,
    tick: Duration,
    ready: Option<Box<ReadyHook>>,
}

impl Daemon {
    /// The actual bound address (the real port when 0 was requested).
    pub fn addr(&self) -> &Addr {
        &self.actual
    }

    /// The session manager: restore diagnostics, tests, shutdown plumbing.
    pub fn manager(&self) -> &Arc<Manager> {
        &self.manager
    }

    /// Start serving: spawn the accept loop and the idle ticker, run the
    /// readiness hook, and return the running daemon.
    pub fn start(mut self) -> RunningDaemon {
        let listener = self.listener;
        let mgr = self.manager.clone();
        let accept = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let m = mgr.clone();
                tokio::spawn(async move {
                    let _ = server::accept(stream, m).await;
                });
            }
        });

        let tick = self.tick;
        let m = self.manager.clone();
        let ticker = tokio::spawn(async move {
            let mut tick = tokio::time::interval(tick);
            tick.tick().await; // skip the immediate first tick (already published)
            loop {
                tick.tick().await;
                for s in m.list() {
                    // Status hooks: ask the harness what the session is
                    // doing. Honest — the adapter returns None when it
                    // cannot tell, and the session only stores what it
                    // reports.
                    if s.status() == Status::Running {
                        let harness = HarnessRegistry::identify(&s.command);
                        let working_copy = s.working_copy();
                        s.set_waiting(harness.waiting_for_input(&working_copy));
                    }
                    s.publish_state();
                }
            }
        });

        if let Some(ready) = self.ready.take() {
            ready(self.actual.clone());
        }
        RunningDaemon {
            actual: self.actual,
            manager: self.manager,
            accept,
            ticker,
            stopped: false,
        }
    }
}

/// A serving daemon. Call [`RunningDaemon::shutdown`] for a graceful stop
/// (kill and flush sessions, join the server tasks); dropping without
/// shutting down aborts the tasks and kills sessions, skipping the flush.
pub struct RunningDaemon {
    actual: Addr,
    manager: Arc<Manager>,
    accept: JoinHandle<()>,
    ticker: JoinHandle<()>,
    stopped: bool,
}

impl RunningDaemon {
    /// The actual bound address.
    pub fn addr(&self) -> &Addr {
        &self.actual
    }

    /// The session manager.
    pub fn manager(&self) -> &Arc<Manager> {
        &self.manager
    }

    /// Resolves when the accept loop exits on its own — a listener failure,
    /// after which the daemon can no longer serve. Pair with a shutdown
    /// signal in a `select!`; does not shut anything down itself.
    pub async fn accept_ended(&mut self) {
        let _ = (&mut self.accept).await;
    }

    /// Graceful shutdown: stop accepting new connections and the ticker,
    /// join both, then kill and flush every session. Consumes `self`, so
    /// it can only run once.
    pub async fn shutdown(mut self) {
        if !self.stopped {
            self.stopped = true;
            self.accept.abort();
            self.ticker.abort();
            let _ = (&mut self.accept).await;
            let _ = (&mut self.ticker).await;
            self.manager.kill_all();
            self.manager.flush_all().await;
        }
    }
}

impl Drop for RunningDaemon {
    /// Best-effort cleanup for a daemon that was dropped rather than shut
    /// down: stop the server tasks and kill sessions so nothing is
    /// orphaned. The event-log flush is async and cannot run here — call
    /// [`RunningDaemon::shutdown`] for that.
    fn drop(&mut self) {
        if !self.stopped {
            self.accept.abort();
            self.ticker.abort();
            self.manager.kill_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::Duration};

    use bao_core::{
        sandbox::{SandboxKind, SandboxSpec},
        types::{Command, TerminalSize},
    };
    use bao_transport::Addr;
    use tokio::sync::oneshot;

    use super::*;

    fn temp_home(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bao-daemon-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sandbox() -> SandboxSpec {
        SandboxSpec {
            isolation: SandboxKind::InPlace,
        }
    }

    /// A daemon over an ephemeral port: build, start, and return it with
    /// the actual address.
    async fn start_test_daemon(label: &str) -> (RunningDaemon, Addr) {
        let root = temp_home(label);
        let daemon = DaemonBuilder::new(Home::new(&root), Addr::local(0))
            .build()
            .await
            .unwrap();
        let addr = daemon.addr().clone();
        (daemon.start(), addr)
    }

    /// The full lifecycle over real TCP: the client round-trips, shutdown
    /// kills the sessions, and the listener is closed.
    #[tokio::test]
    async fn serves_clients_and_shuts_down_cleanly() {
        let (running, addr) = start_test_daemon("lifecycle").await;
        let manager = running.manager().clone();

        let mut conn = bao_client::Conn::connect(&addr).await.unwrap();
        assert!(conn.list().await.unwrap().is_empty());

        let meta = conn
            .launch(
                Some(Command::parse("bash -c 'sleep 30'").unwrap()),
                Some(temp_home("lifecycle-cwd")),
                None,
                None,
                None,
                TerminalSize::default(),
                sandbox(),
            )
            .await
            .unwrap();
        assert_eq!(manager.list().len(), 1);
        assert_eq!(manager.list()[0].id, meta.id);

        // Graceful shutdown: kills and flushes sessions, closes the listener.
        running.shutdown().await;

        // The session process is gone (kill_all ran).
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let all_exited = manager
                .list()
                .iter()
                .all(|s| matches!(s.status(), Status::Exited(_)));
            if all_exited {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "sessions were not killed by shutdown"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // The listener is gone: new connections are refused.
        assert!(
            bao_client::Conn::connect(&addr).await.is_err(),
            "port must be closed after shutdown"
        );
    }

    /// The readiness hook fires exactly once, with the actual bound address
    /// (port 0 resolved to a real port).
    #[tokio::test]
    async fn readiness_hook_receives_the_actual_addr() {
        let root = temp_home("ready");
        let (tx, rx) = oneshot::channel();
        let daemon = DaemonBuilder::new(Home::new(&root), Addr::local(0))
            .on_ready(move |addr| {
                let _ = tx.send(addr);
            })
            .build()
            .await
            .unwrap();
        let actual = daemon.addr().clone();
        let running = daemon.start();

        let hooked = rx.await.unwrap();
        assert_eq!(hooked, actual);
        assert_ne!(hooked, Addr::local(0), "ephemeral port must be resolved");

        running.shutdown().await;
    }

    /// Build fails fast on a taken port — before anything is serving.
    #[tokio::test]
    async fn build_fails_fast_when_the_port_is_taken() {
        let root = temp_home("port");
        let daemon = DaemonBuilder::new(Home::new(&root), Addr::local(0))
            .build()
            .await
            .unwrap();
        let addr = daemon.addr().clone();

        let result = DaemonBuilder::new(Home::new(&root), addr).build().await;
        assert!(matches!(result, Err(Error::Io(_))));

        let running = daemon.start();
        running.shutdown().await;
    }

    /// Unix sockets aren't wired up yet — refused at build, not at connect
    /// time.
    #[tokio::test]
    async fn unix_addresses_are_rejected_at_build() {
        let root = temp_home("unix");
        let result = DaemonBuilder::new(Home::new(&root), Addr::unix(root.join("sock")))
            .build()
            .await;
        assert!(matches!(
            result,
            Err(Error::TransportUnsupported("unix socket"))
        ));
    }
}
