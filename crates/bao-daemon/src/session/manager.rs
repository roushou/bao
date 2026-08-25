//! The session registry + the launch saga.

use super::*;
use crate::home::Home;
use crate::sandbox::{RealSandboxFactory, SandboxFactory};

/// One entry on the state bus: a session's current picture, or a
/// removal. Watchers render this as-is — they never derive it.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum StateEvent {
    Snapshot(SessionMeta),
    Gone {
        session: SessionId,
        reason: Option<String>,
    },
}

/// Registry of sessions known to this host.
pub struct Manager {
    sessions: RwLock<HashMap<SessionId, Arc<Session>>>,
    working_copies: WorkingCopyStore,
    /// Registered launch targets (`workspaces.json`). Sessions are aimed at
    /// workspaces; the registry is daemon-side because a path only means
    /// something on the host that can see it.
    workspaces: RwLock<crate::workspace::WorkspaceRegistry>,
    /// On-disk session store (versioned, atomically written, salvage-on-
    /// restore).
    store: SessionStore,
    /// State bus: every session's derived picture (and removals), broadcast
    /// so any overview can stream all sessions over one connection without
    /// polling or per-session subscriptions.
    state_bus: broadcast::Sender<StateEvent>,
    /// Materializes sandboxes for launches. The production manager uses the
    /// real dispatcher; tests inject a fake so the registry and saga are
    /// testable without git, disk, or sandbox binaries.
    sandbox_factory: Arc<dyn SandboxFactory>,
}

impl Manager {
    /// New registry rooted at the given directories (used by tests and the
    /// composition root; `open` derives them from a [`Home`]).
    pub fn new(sessions_dir: PathBuf, working_copies_dir: PathBuf) -> Self {
        Self::with_store(
            sessions_dir,
            WorkingCopyStore::new(working_copies_dir),
            Arc::new(RealSandboxFactory),
        )
    }

    fn with_store(
        sessions_dir: PathBuf,
        working_copies: WorkingCopyStore,
        sandbox_factory: Arc<dyn SandboxFactory>,
    ) -> Self {
        std::fs::create_dir_all(&sessions_dir).ok();
        let (state_bus, _) = broadcast::channel(1024);
        Manager {
            sessions: RwLock::new(HashMap::new()),
            working_copies,
            workspaces: RwLock::new(crate::workspace::WorkspaceRegistry::in_memory()),
            store: SessionStore::new(sessions_dir),
            state_bus,
            sandbox_factory,
        }
    }

    /// Fresh registry from the bao home, then restore any sessions on disk.
    pub fn open(home: &Home) -> Result<Self, Error> {
        Self::open_with(home, Arc::new(RealSandboxFactory))
    }

    /// Like [`Manager::open`], but with an injected sandbox factory (tests).
    pub(crate) fn open_with(
        home: &Home,
        sandbox_factory: Arc<dyn SandboxFactory>,
    ) -> Result<Self, Error> {
        let m = Self::with_store(
            home.sessions_dir(),
            WorkingCopyStore::new(home.working_copies_dir()),
            sandbox_factory,
        );
        *m.workspaces.write().unwrap() = crate::workspace::WorkspaceRegistry::load(home.root())?;
        m.load_existing()?;
        Ok(m)
    }

    /// The sessions data directory.
    pub fn sessions_dir(&self) -> &Path {
        self.store.dir()
    }

    /// The workspace registry (read): resolve aliases, list targets.
    pub fn workspaces(
        &self,
    ) -> std::sync::RwLockReadGuard<'_, crate::workspace::WorkspaceRegistry> {
        self.workspaces.read().unwrap()
    }

    /// The workspace registry (write): register and forget launch targets.
    pub(crate) fn workspaces_mut(
        &self,
    ) -> std::sync::RwLockWriteGuard<'_, crate::workspace::WorkspaceRegistry> {
        self.workspaces.write().unwrap()
    }

    fn load_existing(&self) -> Result<(), Error> {
        let restored = Session::restore_all(&self.store)?;
        for s in restored {
            let working_copy = s.working_copy();
            s.set_sandbox(Sandbox::from_workspace(&self.working_copies, working_copy));
            self.spawn_state_forwarder(&s);
            self.sessions.write().unwrap().insert(s.id.clone(), s);
        }
        Ok(())
    }

    /// Register a session as `Preparing`: generate the id, persist identity,
    /// make it visible to watchers. No sandbox, no process yet — the launch
    /// saga materializes those.
    fn begin_launch(
        &self,
        command: &Command,
        cwd: &Path,
        size: TerminalSize,
        name: Option<String>,
    ) -> Result<Arc<Session>, Error> {
        let id = SessionId::generate();
        let provisional = WorkingCopy {
            kind: SandboxKind::InPlace,
            repo: None,
            branch: None,
            path: cwd.to_path_buf(),
        };
        let spec = SessionSpec {
            id,
            name,
            command: command.clone(),
            working_copy: provisional,
            size,
            clock: Clock::system(),
        };
        let s = Session::register(&spec, &self.store)?;
        self.spawn_state_forwarder(&s);
        self.sessions
            .write()
            .unwrap()
            .insert(s.id.clone(), s.clone());
        Ok(s)
    }

    /// Launch a session synchronously (tests, and any caller that wants
    /// blocking behavior): register, build the sandbox, spawn — and on any
    /// failure, compensate and forget, so nothing leaks.
    pub fn create(
        &self,
        command: &Command,
        cwd: &Path,
        size: TerminalSize,
        name: Option<String>,
        sandbox: &SandboxSpec,
    ) -> Result<Arc<Session>, Error> {
        let sess = self.begin_launch(command, cwd, size, name)?;
        let sid = sess.id.clone();
        let result = self
            .sandbox_factory
            .materialize(&self.working_copies, &sid, cwd, sandbox)
            .and_then(|sb| self.attach_and_spawn(&sess, command, size, sb));
        if let Err(e) = result {
            self.fail_launch(&sid, Some(e.to_string()));
            return Err(e);
        }
        Ok(sess)
    }

    /// Launch a session in the background: register it as `Preparing` and
    /// return immediately; the two-step saga (sandbox → spawn) runs in a
    /// spawned task and watchers see it advance — or roll back via
    /// [`StateEvent::Gone`].
    pub async fn launch(
        self: &Arc<Self>,
        command: Command,
        cwd: PathBuf,
        size: TerminalSize,
        name: Option<String>,
        sandbox: SandboxSpec,
    ) -> Result<Arc<Session>, Error> {
        let sess = self.begin_launch(&command, &cwd, size, name)?;
        let m = self.clone();
        let task_sess = sess.clone();
        tokio::spawn(async move {
            let sid = task_sess.id.clone();
            let sid_for_sandbox = sid.clone();
            let working_copies = m.working_copies.clone();
            let factory = m.sandbox_factory.clone();
            // The blocking git worktree step runs off the async runtime.
            let sandbox_result = tokio::task::spawn_blocking(move || {
                factory.materialize(&working_copies, &sid_for_sandbox, &cwd, &sandbox)
            })
            .await;
            let sb = match sandbox_result {
                Ok(Ok(sb)) => sb,
                Ok(Err(e)) => {
                    m.fail_launch(&sid, Some(e.to_string()));
                    return;
                }
                Err(e) => {
                    m.fail_launch(&sid, Some(format!("sandbox task panicked: {e}")));
                    return;
                }
            };
            if let Err(e) = m.attach_and_spawn(&task_sess, &command, size, sb) {
                m.fail_launch(&sid, Some(e.to_string()));
            }
        });
        Ok(sess)
    }

    /// The saga's materialize-and-spawn core: set the real sandbox, then
    /// spawn the process (Preparing → Starting).
    fn attach_and_spawn(
        &self,
        sess: &Arc<Session>,
        command: &Command,
        size: TerminalSize,
        sandbox: Sandbox,
    ) -> Result<(), Error> {
        sess.set_workspace(sandbox.working_copy.clone());
        sess.set_sandbox(sandbox);
        sess.start_process(command, size)?;
        Ok(())
    }

    /// Compensate a failed launch: kill any process, remove the sandbox, and
    /// forget the session — broadcasting `Gone` so every watcher drops it.
    fn fail_launch(&self, sid: &SessionId, reason: Option<String>) {
        if let Ok(sess) = self.resolve(sid.as_str()) {
            let _ = sess.kill();
            let _ = sess.teardown_sandbox();
        }
        let _ = self.store.remove_dir(sid);
        self.sessions.write().unwrap().remove(sid);
        let _ = self.state_bus.send(StateEvent::Gone {
            session: sid.clone(),
            reason,
        });
    }

    /// Bridge one session's state channel onto the state bus. Pushes
    /// the current picture once (so watchers that joined before a session
    /// existed still catch it), then every change. Runs until the session is
    /// dropped; never exits because the bus has no listeners — a later
    /// watcher must see changes too.
    fn spawn_state_forwarder(&self, sess: &Arc<Session>) {
        let bus = self.state_bus.clone();
        let mut rx = sess.state_subscribe();
        let _ = bus.send(StateEvent::Snapshot(sess.meta()));
        // The daemon always runs inside tokio; if no runtime is present
        // (tests, future embedders), skip the forwarder — the bus simply
        // has nothing to say.
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        handle.spawn(async move {
            loop {
                match rx.changed().await {
                    Ok(_) => {
                        let m = rx.borrow().clone();
                        // Err means "no subscribers right now" — keep
                        // forwarding regardless.
                        let _ = bus.send(StateEvent::Snapshot(m));
                    }
                    Err(_) => return, // session dropped
                }
            }
        });
    }

    /// Subscribe to the state stream: every session's derived picture
    /// and every removal, over one channel.
    pub fn subscribe_state(&self) -> broadcast::Receiver<StateEvent> {
        self.state_bus.subscribe()
    }

    /// Resolve a session by exact id, exact name, or a unique prefix of
    /// either. The input is user-typed, so it stays a string; the matching is
    /// typed on the inside.
    pub fn resolve(&self, id_or_name: &str) -> Result<Arc<Session>, Error> {
        let map = self.sessions.read().unwrap();
        if let Some(s) = map.values().find(|s| s.id.as_str() == id_or_name) {
            return Ok(s.clone());
        }
        if let Some(s) = map
            .values()
            .find(|s| s.name.lock().unwrap().as_deref() == Some(id_or_name))
        {
            return Ok(s.clone());
        }
        let id_matches: Vec<&SessionId> = map
            .keys()
            .filter(|k| k.matches_prefix(id_or_name))
            .collect();
        let name_matches: Vec<Arc<Session>> = map
            .values()
            .filter(|s| {
                s.name
                    .lock()
                    .unwrap()
                    .as_deref()
                    .is_some_and(|n| n.starts_with(id_or_name))
            })
            .cloned()
            .collect();
        match (id_matches.len(), name_matches.len()) {
            (1, 0) => Ok(map.get(id_matches[0]).unwrap().clone()),
            (0, 1) => Ok(name_matches[0].clone()),
            (0, 0) => Err(Error::NotFound(id_or_name.to_string())),
            (a, b) => Err(Error::Ambiguous(id_or_name.to_string(), a, b)),
        }
    }

    pub fn list(&self) -> Vec<Arc<Session>> {
        let mut v: Vec<_> = self.sessions.read().unwrap().values().cloned().collect();
        v.sort_by_key(|s| s.created);
        v
    }

    /// Stop the session (if any), remove its worktree, and forget the session.
    pub fn remove(&self, id_or_name: &str) -> Result<(), Error> {
        let sess = self.resolve(id_or_name)?;
        let id = sess.id.clone();
        let _ = sess.kill();
        sess.teardown_sandbox()?;
        let _ = self.store.remove_dir(&id);
        self.sessions.write().unwrap().remove(&id);
        let _ = self.state_bus.send(StateEvent::Gone {
            session: id,
            reason: None,
        });
        Ok(())
    }

    pub fn kill_all(&self) {
        for s in self.list() {
            let _ = s.kill();
        }
    }

    /// fsync every session's event log (graceful shutdown).
    pub async fn flush_all(&self) {
        for s in self.list() {
            s.flush_log().await;
        }
    }
}
