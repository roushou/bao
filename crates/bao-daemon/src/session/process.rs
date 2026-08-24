//! The [`Session`]: one live session process + its event log.

use std::io::Read;

use super::log::{Durability, SessionLog, output_snippet};
use super::store::{LoadedLog, RestoredIdentity, StoredMeta};
use super::*;
use crate::sandbox::Sandbox;
use bao_core::types::Hostname;

pub struct Session {
    pub id: SessionId,
    pub(crate) name: Mutex<Option<String>>,
    /// The exact argv the session runs with.
    pub command: Command,
    /// The isolated working copy the session runs in. Mutated when the launch
    /// saga materializes the workspace (provisional → real); everything else
    /// reads it through [`Session::workspace`].
    workspace: Mutex<Workspace>,
    /// The materialized sandbox this session runs in (`None` before the
    /// launch saga materializes it, or for a session restored without a
    /// rehydrated backend).
    sandbox: Mutex<Option<Sandbox>>,
    pub created: u64,
    status: Mutex<Status>,
    /// The event log: in-memory ring, sequence, live-event broadcast, and
    /// async writer. The attach-replay source.
    log: SessionLog,
    /// The live PTY process (master, writer, child) — empty when restored or
    /// exited.
    process: Mutex<Process>,
    /// The store this session persists to and was loaded from.
    store: SessionStore,
    /// Epoch ms of the last session output (0 = none yet). Drives the honest
    /// "idle" signal.
    last_activity: Mutex<u64>,
    /// Cached "what it last said" — the newest output chunk, ANSI-stripped
    /// and one-lined. Updated on append so `meta()` never scans the log.
    last_output: Mutex<String>,
    /// The terminal's current state (the screen), always fed — the daemon
    /// holds it so clients can attach without replaying history.
    screen: Mutex<vt_screen::Screen>,
    /// Machine this session lives on.
    host: Hostname,
    /// Time source for this session's derivations.
    clock: Clock,
    /// Honest "is the session waiting for the human?" — set only by the
    /// daemon's harness poll; `None` = we cannot tell. Cleared when the
    /// session stops.
    waiting: Mutex<Option<bool>>,
    /// The latest derived picture of this session. Stateless views subscribe
    /// here — every value is the complete current state, so a view holds
    /// nothing else. `watch` coalesces: bursts of output never flood clients.
    state_tx: watch::Sender<SessionMeta>,
    /// Last picture we actually pushed, so `publish_state` only sends on
    /// change.
    last_state: Mutex<Option<SessionMeta>>,
}

/// One live PTY process: the master handle, its writer, and the child — the
/// three handles that always appear and disappear together. Empty while the
/// session has no live process (restored, exited, not yet spawned).
#[derive(Default)]
struct Process {
    master: Option<Box<dyn MasterPty + Send>>,
    writer: Option<Box<dyn Write + Send>>,
    child: Option<Box<dyn Child + Send + Sync>>,
}

impl Process {
    /// Open the PTY and spawn `command` in `workspace`, sandbox applied.
    /// Returns the master reader for the caller's pump.
    fn open(
        &mut self,
        command: &Command,
        workspace: &Workspace,
        sandbox: Option<&Sandbox>,
        size: TerminalSize,
    ) -> Result<Box<dyn Read + Send>, Error> {
        if self.child.is_some() {
            return Err(Error::AlreadyRunning);
        }
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: size.rows,
                cols: size.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| Error::Pty(e.to_string()))?;
        let mut cmd = CommandBuilder::from_argv(
            command
                .as_args()
                .iter()
                .map(|a| std::ffi::OsString::from(a.as_str()))
                .collect(),
        );
        cmd.cwd(&workspace.path);
        cmd.env("TERM", "xterm-256color");
        if let Some(sb) = sandbox {
            sb.wrap_command(&mut cmd)?;
        }
        let child = pair.slave.spawn_command(cmd).map_err(|e| {
            Error::Spawn(format!(
                "failed to spawn the harness (is it installed?): {e}"
            ))
        })?;
        drop(pair.slave);
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| Error::Pty(e.to_string()))?;
        self.writer = Some(
            pair.master
                .take_writer()
                .map_err(|e| Error::Pty(e.to_string()))?,
        );
        self.master = Some(pair.master);
        self.child = Some(child);
        Ok(reader)
    }

    /// Write bytes to the process's stdin. [`Error::NotRunning`] when there
    /// is no live process.
    fn input(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let w = self.writer.as_mut().ok_or(Error::NotRunning)?;
        w.write_all(bytes)?;
        w.flush()?;
        Ok(())
    }

    /// Resize the PTY window. [`Error::NotRunning`] when there is none.
    fn resize(&mut self, size: TerminalSize) -> Result<(), Error> {
        let m = self.master.as_mut().ok_or(Error::NotRunning)?;
        m.resize(PtySize {
            rows: size.rows,
            cols: size.cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| Error::Pty(e.to_string()))
    }

    /// Kill the child. A no-op when there is no live process.
    fn kill(&mut self) -> Result<(), Error> {
        if let Some(c) = self.child.as_mut() {
            c.kill().map_err(|e| Error::Pty(e.to_string()))?;
        }
        Ok(())
    }

    /// Take the child for the pump's exit-wait (fires on master EOF).
    fn take_child(&mut self) -> Option<Box<dyn Child + Send + Sync>> {
        self.child.take()
    }
}

impl Session {
    /// Register a session (identity + a provisional workspace) at `Preparing` —
    /// no process yet. The launch saga materializes the workspace and spawns.
    pub(crate) fn register(spec: &SessionSpec, store: &SessionStore) -> Result<Arc<Self>, Error> {
        if spec.command.is_empty() {
            return Err(bao_core::error::Error::EmptyCommand.into());
        }
        let data_dir = store.dir().join(spec.id.as_str());
        std::fs::create_dir_all(&data_dir).ok();
        let clock = spec.clock;
        let created = clock.now_ms();
        let initial = SessionMeta {
            id: spec.id.clone(),
            name: spec.name.clone(),
            command: spec.command.display(),
            args: spec.command.clone(),
            cwd: spec.workspace.path.clone(),
            workspace: spec.workspace.clone(),
            created,
            host: crate::hostname::resolve(),
            status: Status::Preparing,
            last_activity: created,
            last_output: String::new(),
            alert: None,
            waiting_for_input: None,
            idle_secs: 0,
            age_secs: 0,
        };
        let (state_tx, _) = watch::channel(initial.clone());

        let sess = Arc::new(Session {
            id: spec.id.clone(),
            name: Mutex::new(spec.name.clone()),
            command: spec.command.clone(),
            workspace: Mutex::new(spec.workspace.clone()),
            sandbox: Mutex::new(None),
            created,
            status: Mutex::new(Status::Preparing),
            log: SessionLog::new(&spec.id, &data_dir, Durability::default()),
            process: Mutex::new(Process::default()),
            store: store.clone(),
            last_activity: Mutex::new(created),
            last_output: Mutex::new(String::new()),
            screen: Mutex::new(vt_screen::Screen::new(spec.size)),
            host: initial.host.clone(),
            clock,
            waiting: Mutex::new(None),
            state_tx,
            last_state: Mutex::new(Some(initial)),
        });
        sess.persist_meta();
        Ok(sess)
    }

    /// Register + spawn in one step (the synchronous launch path, used by
    /// tests).
    #[cfg(test)]
    pub(crate) fn spawn(spec: &SessionSpec, store: &SessionStore) -> Result<Arc<Self>, Error> {
        let sess = Self::register(spec, store)?;
        sess.start_process(&spec.command, spec.size)?;
        Ok(sess)
    }

    /// Rename this session (`None` clears the name). Persisted, and the new
    /// picture is pushed to every watcher via the state bus.
    pub fn rename(&self, name: Option<String>) {
        *self.name.lock().unwrap() = name;
        self.persist_meta();
        self.publish_state();
    }

    /// Relaunch the session for a session whose process was lost — same env,
    /// same session, same log. Continues the event-log sequence numbers.
    pub fn resume(self: &Arc<Self>, command: &Command, size: TerminalSize) -> Result<(), Error> {
        if self.status() != Status::Interrupted {
            return Err(Error::ResumeNotInterrupted(self.status()));
        }
        self.start_process(command, size)?;
        // The PTY was opened at `size`; bring the snapshot screen to the same
        // size so screen snapshots and the PTY window never disagree.
        self.screen.lock().unwrap().resize(size);
        Ok(())
    }

    /// Spawn the session process in a PTY inside the environment's working
    /// copy, then pump its output into the log. Shared by `spawn` and
    /// `resume`.
    pub(crate) fn start_process(
        self: &Arc<Self>,
        command: &Command,
        size: TerminalSize,
    ) -> Result<(), Error> {
        let workspace = self.workspace();
        let mut reader = {
            let sandbox = self.sandbox.lock().unwrap();
            let mut process = self.process.lock().unwrap();
            process.open(command, &workspace, sandbox.as_ref(), size)?
        };
        // The process is up — Preparing → Starting on launch, Interrupted →
        // Starting on resume. Emitted before the pump starts so the first
        // output sees `Starting` and flips it to `Running`.
        self.transition(LifecycleEvent::Spawned)?;

        // Pump the PTY master (blocking std Read) into a tokio channel.
        let (chunk_tx, mut chunk_rx) = tokio::sync::mpsc::channel::<Option<Vec<u8>>>(256);
        std::thread::spawn(move || {
            let mut buf = vec![0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if chunk_tx.blocking_send(Some(buf[..n].to_vec())).is_err() {
                            return;
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = chunk_tx.blocking_send(None);
        });

        let s2 = self.clone();
        tokio::spawn(async move {
            while let Some(Some(chunk)) = chunk_rx.recv().await {
                s2.append(EventKind::Output(chunk));
            }
            // EOF on the master: wait for the child and record its exit.
            let code = {
                let child = s2.process.lock().unwrap().take_child();
                match child {
                    Some(mut ch) => match tokio::task::spawn_blocking(move || ch.wait()).await {
                        Ok(Ok(st)) => Some(st.exit_code() as i32),
                        _ => None,
                    },
                    None => None,
                }
            };
            let _ = s2.transition(LifecycleEvent::Exited(code));
        });
        Ok(())
    }

    /// Append an event: log it (ring buffer), persist it, broadcast it.
    fn append(&self, kind: EventKind) {
        // First output flips a booting session to running — the boot-complete
        // fact — before the output itself is recorded, so the log orders
        // `Status(Running)` ahead of the first `Output`.
        if matches!(kind, EventKind::Output(_)) && self.status() == Status::Starting {
            let _ = self.transition(LifecycleEvent::Output);
        }

        let ts = self.clock.now_ms();
        // Output: the sequence number and the screen update happen together
        // under the screen lock, so `attach_point` can capture a consistent
        // (seq, snapshot) pair — the snapshot always reflects exactly the
        // output with seq <= the returned value. The seq itself lives in the
        // log; it is assigned here, under the screen lock.
        match &kind {
            EventKind::Output(bytes) => {
                let mut screen = self.screen.lock().unwrap();
                self.log.append(kind.clone(), ts);
                screen.process(bytes);
            }
            EventKind::Status(_) => {
                self.log.append(kind.clone(), ts);
            }
        }
        if let EventKind::Output(bytes) = &kind {
            *self.last_activity.lock().unwrap() = ts;
            *self.last_output.lock().unwrap() = output_snippet(bytes);
        }
        self.publish_state();
    }

    /// Drive the session's lifecycle one event forward — the only write path
    /// for status. Records the change as a persisted event; `meta.json` is not
    /// updated (the log is the source of truth for state).
    fn transition(&self, event: LifecycleEvent) -> Result<(), Error> {
        let next = self.status().apply(&event)?;
        if next == self.status() {
            return Ok(());
        }
        *self.status.lock().unwrap() = next;
        if next != Status::Running {
            // A stopped session is never "waiting for you".
            *self.waiting.lock().unwrap() = None;
        }
        self.append(EventKind::Status(next));
        Ok(())
    }

    pub fn status(&self) -> Status {
        *self.status.lock().unwrap()
    }

    /// The session's current workspace (a clone — the working copy may change
    /// as the launch saga materializes it).
    pub fn workspace(&self) -> Workspace {
        self.workspace.lock().unwrap().clone()
    }

    /// Replace the workspace — the launch saga's step 1 materializes the real
    /// working copy over the provisional one, then re-persists and publishes.
    pub fn set_workspace(&self, workspace: Workspace) {
        {
            let mut cur = self.workspace.lock().unwrap();
            *cur = workspace;
        }
        self.persist_meta();
        self.publish_state();
    }

    /// Attach the materialized sandbox (its backend may hold runtime state).
    pub(crate) fn set_sandbox(&self, sandbox: Sandbox) {
        *self.sandbox.lock().unwrap() = Some(sandbox);
    }

    /// Tear the session's sandbox down. A no-op when there is none.
    pub(crate) fn teardown_sandbox(&self) -> Result<(), Error> {
        if let Some(sandbox) = self.sandbox.lock().unwrap().as_ref() {
            sandbox.teardown()?;
        }
        Ok(())
    }

    /// A consistent (log sequence, screen snapshot) pair for attach. The
    /// contract — and its locking requirement — lives on
    /// [`Screen::snapshot`](vt_screen::Screen::snapshot); the screen lock
    /// held here is the one `append` holds when it advances the sequence for
    /// output.
    pub fn attach_point(&self) -> (u64, Vec<u8>) {
        let screen = self.screen.lock().unwrap();
        let seq = self.log.last_seq();
        screen.snapshot(seq)
    }

    /// The session's terminal size, `(rows, cols)` — what a client emulator
    /// must render at.
    pub fn screen_size(&self) -> (u16, u16) {
        self.screen.lock().unwrap().size()
    }

    /// Write bytes to the session's terminal (stdin).
    pub fn input(&self, bytes: &[u8]) -> Result<(), Error> {
        self.process.lock().unwrap().input(bytes)
    }

    pub fn resize(&self, size: TerminalSize) -> Result<(), Error> {
        // Lock order: screen, then process. `append` takes only `screen`, so
        // this order cannot deadlock with it. The PTY window and the
        // snapshot screen change together to the same size — the single
        // size-mutation point (see `Screen::resize`).
        let mut screen = self.screen.lock().unwrap();
        self.process.lock().unwrap().resize(size)?;
        screen.resize(size);
        Ok(())
    }

    pub fn kill(&self) -> Result<(), Error> {
        self.process.lock().unwrap().kill()
    }

    /// fsync the event log so everything appended so far is durable. Called
    /// on graceful daemon shutdown; a crash skips it (restore reflects what
    /// reached disk).
    pub async fn flush_log(&self) {
        self.log.flush().await;
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.log.subscribe()
    }

    /// Snapshot the log entries with seq > `after`, plus the latest seq,
    /// under one lock so attach-replay cannot race with new appends.
    pub fn snapshot_and_last(&self, after: u64) -> (Vec<SessionEvent>, u64) {
        self.log.snapshot_after(after)
    }

    /// Seconds since the session last produced output — the daemon's time
    /// fact, stamped with the injected clock.
    pub fn idle_secs(&self) -> u64 {
        idle_secs(*self.last_activity.lock().unwrap(), self.clock.now_ms())
    }

    /// Does this session need a human right now, and why? Derived from the
    /// session's own facts — status and the daemon's idle measurement.
    pub fn alert(&self) -> Option<Alert> {
        AlertInput {
            status: self.status(),
            idle_secs: self.idle_secs(),
        }
        .alert()
    }

    /// The typed, complete snapshot of this session — identity plus the
    /// server-derived facts (alert, waiting-for-input, time). The daemon
    /// is the only place this is computed; views render it as-is.
    pub fn meta(&self) -> SessionMeta {
        let now = self.clock.now_ms();
        let last_activity = *self.last_activity.lock().unwrap();
        let status = self.status();
        let workspace = self.workspace();
        SessionMeta {
            id: self.id.clone(),
            name: self.name.lock().unwrap().clone(),
            command: self.command.display(),
            args: self.command.clone(),
            cwd: workspace.path.clone(),
            workspace,
            created: self.created,
            host: self.host.clone(),
            status,
            last_activity,
            last_output: self.last_output.lock().unwrap().clone(),
            alert: self.alert(),
            waiting_for_input: *self.waiting.lock().unwrap(),
            idle_secs: idle_secs(last_activity, now),
            age_secs: now.saturating_sub(self.created) / 1000,
        }
    }

    /// Store the harness's honest answer to "is it waiting for the human?"
    /// and publish it (only when it actually changed). Called by the daemon's
    /// ticker via the adapter — core never guesses; it only stores.
    pub fn set_waiting(&self, waiting: Option<bool>) {
        let mut cur = self.waiting.lock().unwrap();
        if *cur != waiting {
            *cur = waiting;
            drop(cur);
            self.publish_state();
        }
    }

    /// Re-derive the current picture and push it to state subscribers — but
    /// only when something actually changed. Called on every session event
    /// and by the daemon's idle ticker, so time-based signals (idle) arrive
    /// without any client polling.
    pub fn publish_state(&self) {
        let m = self.meta();
        let mut last = self.last_state.lock().unwrap();
        if last.as_ref() != Some(&m) {
            *last = Some(m.clone());
            let _ = self.state_tx.send(m);
        }
    }

    /// Watch channel of the latest complete picture. A stateless view holds
    /// nothing else: every value is the whole current state, and
    /// `changed()` coalesces bursts of output into the newest value.
    pub fn state_subscribe(&self) -> watch::Receiver<SessionMeta> {
        self.state_tx.subscribe()
    }

    fn persist_meta(&self) {
        let _ = self
            .store
            .write_meta(&self.id, &StoredMeta::from_session(self));
    }

    /// Rebuild sessions from on-disk state after a daemon restart. A session
    /// whose meta says `exited` stays exited; anything else that was running
    /// is honestly marked [`Status::Interrupted`]. A session whose
    /// `meta.json` can't be read (corrupt, or from a newer Bao) is salvaged
    /// as [`Status::Damaged`] — its log is kept, so history stays viewable
    /// and it can be removed. Only directories with *no* `meta.json` are
    /// skipped (not a session dir).
    pub(crate) fn restore_all(store: &SessionStore) -> Result<Vec<Arc<Self>>, Error> {
        let mut out = Vec::new();
        let entries = match std::fs::read_dir(store.dir()) {
            Ok(e) => e,
            Err(_) => return Ok(out),
        };
        for entry in entries.flatten() {
            let data_dir = entry.path();
            if !data_dir.is_dir() {
                continue;
            }
            let Some(id) = data_dir.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let id = SessionId::from_str(id).unwrap_or_default();
            let loaded = store.load_log(&id);
            if loaded.corrupt_lines > 0 {
                eprintln!(
                    "bao daemon: session {id}: {} corrupt event line(s) skipped",
                    loaded.corrupt_lines
                );
            }
            match store.read_meta(&id) {
                Ok(Some(stored)) => {
                    let cwd = stored.cwd.unwrap_or_default();
                    let command = if stored.args.is_empty() {
                        Command::parse(&stored.command).unwrap_or_default()
                    } else {
                        Command::from_args(stored.args)
                    };
                    let workspace = stored.workspace.unwrap_or(Workspace {
                        kind: SandboxKind::InPlace,
                        repo: None,
                        branch: None,
                        path: cwd.clone(),
                    });
                    let status = fold_status(&loaded.log);
                    out.push(Self::build_restored(
                        store,
                        id,
                        RestoredIdentity {
                            name: stored.name,
                            command,
                            workspace,
                            created: stored.created,
                            status,
                        },
                        loaded,
                    ));
                }
                // No meta.json: not a session dir.
                Ok(None) => {}
                // Unreadable meta: salvage — keep the log, surface honestly.
                Err(_) => {
                    let workspace = Workspace {
                        kind: SandboxKind::InPlace,
                        repo: None,
                        branch: None,
                        path: data_dir.clone(),
                    };
                    out.push(Self::build_restored(
                        store,
                        id,
                        RestoredIdentity {
                            name: None,
                            command: Command::default(),
                            workspace,
                            created: if loaded.first_ts > 0 {
                                loaded.first_ts
                            } else {
                                now_ms()
                            },
                            status: Status::Damaged,
                        },
                        loaded,
                    ));
                }
            }
        }
        Ok(out)
    }

    /// Build a restored session from a parsed identity + log. Shared by the
    /// normal and salvaged (`Damaged`) paths.
    fn build_restored(
        store: &SessionStore,
        id: SessionId,
        identity: RestoredIdentity,
        loaded: LoadedLog,
    ) -> Arc<Self> {
        let RestoredIdentity {
            name,
            command,
            workspace,
            created,
            status,
        } = identity;
        let snippet = loaded
            .last_output
            .as_deref()
            .map(output_snippet)
            .unwrap_or_default();
        // Reconstruct the screen from the log (once, at restore) so an
        // interrupted session attaches to its real last state.
        let mut restored_screen = vt_screen::Screen::new(TerminalSize::default());
        for (_, kind) in loaded.log.iter() {
            if let EventKind::Output(bytes) = kind {
                restored_screen.process(bytes);
            }
        }
        let initial = SessionMeta {
            id: id.clone(),
            name: name.clone(),
            command: command.display(),
            args: command.clone(),
            cwd: workspace.path.clone(),
            workspace: workspace.clone(),
            created,
            host: crate::hostname::resolve(),
            status,
            last_activity: loaded.last_ts,
            last_output: snippet.clone(),
            alert: AlertInput {
                status,
                idle_secs: idle_secs(loaded.last_ts, now_ms()),
            }
            .alert(),
            waiting_for_input: None,
            idle_secs: idle_secs(loaded.last_ts, now_ms()),
            age_secs: now_ms().saturating_sub(created) / 1000,
        };
        let (state_tx, _) = watch::channel(initial.clone());
        Arc::new(Session {
            log: SessionLog::restored(&id, loaded.log, loaded.last_seq),
            id,
            name: Mutex::new(name),
            command,
            workspace: Mutex::new(workspace),
            sandbox: Mutex::new(None),
            created,
            status: Mutex::new(status),
            process: Mutex::new(Process::default()),
            store: store.clone(),
            last_activity: Mutex::new(loaded.last_ts),
            last_output: Mutex::new(snippet),
            screen: Mutex::new(restored_screen),
            host: initial.host.clone(),
            clock: Clock::system(),
            waiting: Mutex::new(None),
            state_tx,
            last_state: Mutex::new(Some(initial)),
        })
    }
}
