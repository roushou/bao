//! The on-disk session store: `meta.json` + `events.log`, versioned and
//! atomically written.

use super::log::{LOG_CAP, b64d};
use super::*;

/// The on-disk home of sessions: one directory per session holding
/// `meta.json` (identity + lifecycle facts) and `events.log` (append-only
/// output history), under one root. Owns the file format — versioned,
/// atomically written — and the salvage policy: a session whose `meta.json`
/// can't be read is surfaced as [`Status::Damaged`], never silently dropped.
///
/// An internal detail of session management: external code drives sessions
/// through [`Manager`].
#[derive(Clone)]
pub(crate) struct SessionStore {
    dir: PathBuf,
}

/// Current on-disk meta format version. Bump on breaking format changes;
/// older files (no `format` field) are assumed to be 1.
const META_FORMAT: u32 = 1;

/// Everything parsed from a session's `events.log`.
pub(crate) struct LoadedLog {
    pub(crate) log: VecDeque<(u64, EventKind)>,
    pub(crate) last_seq: u64,
    /// Timestamp of the first entry (proxy for session creation when meta
    /// is unreadable).
    pub(crate) first_ts: u64,
    /// Timestamp of the last entry (proxy for last activity).
    pub(crate) last_ts: u64,
    /// Raw bytes of the newest output chunk.
    pub(crate) last_output: Option<Vec<u8>>,
}

/// The parsed-on-disk identity of one session: from a readable `meta.json`,
/// or honest defaults when the meta is salvaged as damaged.
pub(crate) struct RestoredIdentity {
    pub(crate) name: Option<String>,
    pub(crate) command: Command,
    pub(crate) workspace: Workspace,
    pub(crate) created: u64,
    pub(crate) status: Status,
}

impl SessionStore {
    pub fn new(dir: PathBuf) -> Self {
        std::fs::create_dir_all(&dir).ok();
        SessionStore { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn dir_for(&self, id: &SessionId) -> PathBuf {
        self.dir.join(id.as_str())
    }

    /// Atomically persist a session's meta: write a temp file, sync, then
    /// rename over the real one. A crash mid-write leaves the old file
    /// intact — never a truncated `meta.json`.
    pub fn write_meta(&self, id: &SessionId, stored: &StoredMeta) -> Result<(), Error> {
        let dir = self.dir_for(id);
        std::fs::create_dir_all(&dir).ok();
        let tmp = dir.join("meta.json.tmp");
        let final_path = dir.join("meta.json");
        {
            let mut f = std::fs::File::create(&tmp)?;
            serde_json::to_writer_pretty(&mut f, stored)?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, &final_path)?;
        Ok(())
    }

    /// Read a session's meta. `Ok(None)` = no `meta.json` (not a session
    /// dir). `Err` = unreadable, unparseable, or written by a newer Bao —
    /// callers must salvage rather than drop.
    pub fn read_meta(&self, id: &SessionId) -> Result<Option<StoredMeta>, Error> {
        let path = self.dir_for(id).join("meta.json");
        let raw = match std::fs::read_to_string(&path) {
            Ok(r) => r,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let stored: StoredMeta = serde_json::from_str(&raw)?;
        if stored.format.is_some_and(|f| f > META_FORMAT) {
            return Err(Error::NewerFormat(stored.format.unwrap()));
        }
        Ok(Some(stored))
    }

    /// Rebuild a session's log from `events.log` (JSONL of output chunks).
    /// Corrupt/partial lines are skipped; a missing log is an empty log.
    pub fn load_log(&self, id: &SessionId) -> LoadedLog {
        let mut log = VecDeque::new();
        let mut last_seq = 0u64;
        let mut first_ts = u64::MAX;
        let mut last_ts = 0u64;
        let mut last_output: Option<Vec<u8>> = None;
        if let Ok(contents) = std::fs::read_to_string(self.dir_for(id).join("events.log")) {
            for line in contents.lines() {
                let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                    continue;
                };
                let Some(seq) = v.get("seq").and_then(|s| s.as_u64()) else {
                    continue;
                };
                if let Some(ts) = v.get("ts").and_then(|t| t.as_u64()) {
                    first_ts = first_ts.min(ts);
                    last_ts = last_ts.max(ts);
                }
                // `kind: "status"` is a lifecycle event; anything else — or a
                // legacy line with no `kind` — is output.
                let kind = if v.get("kind").and_then(|k| k.as_str()) == Some("status") {
                    let st = v
                        .get("status")
                        .cloned()
                        .and_then(|s| serde_json::from_value::<Status>(s).ok());
                    match st {
                        Some(st) => EventKind::Status(st),
                        None => continue,
                    }
                } else {
                    let Some(data) = v.get("data").and_then(|d| d.as_str()) else {
                        continue;
                    };
                    let Ok(bytes) = b64d(data) else {
                        continue;
                    };
                    last_output = Some(bytes.clone());
                    EventKind::Output(bytes)
                };
                if log.len() >= LOG_CAP {
                    log.pop_front();
                }
                log.push_back((seq, kind));
                last_seq = last_seq.max(seq);
            }
        }
        LoadedLog {
            log,
            last_seq,
            first_ts: if first_ts == u64::MAX { 0 } else { first_ts },
            last_ts,
            last_output,
        }
    }

    /// Remove a session's directory entirely (files and all).
    pub fn remove_dir(&self, id: &SessionId) -> std::io::Result<()> {
        std::fs::remove_dir_all(self.dir_for(id))
    }
}

/// Lenient on-disk session record (old files lack newer fields). Also the
/// serialized shape of `meta.json` (now versioned and written atomically by
/// [`SessionStore`]).
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub(crate) struct StoredMeta {
    /// On-disk format version (missing = 1, written before versioning).
    format: Option<u32>,
    /// (`session` still parses, for meta.json files written before the
    /// harness terminology pass.)
    #[serde(alias = "session")]
    pub(crate) harness: String,
    pub(crate) args: Vec<String>,
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) created: u64,
    pub(crate) status: Option<Status>,
    /// Legacy: pre-embedding records carried the exit code separately.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) exit_code: Option<i32>,
    pub(crate) name: Option<String>,
    /// (`env` still parses, for meta.json files written before the rename.)
    #[serde(alias = "env")]
    pub(crate) workspace: Option<Workspace>,
}

impl StoredMeta {
    /// The persisted record for a live session: identity + lifecycle facts.
    /// Derived state (alert, snippets) is deliberately not persisted —
    /// it is re-derived from the log on restore.
    pub(crate) fn from_session(s: &Session) -> Self {
        let workspace = s.workspace();
        StoredMeta {
            format: Some(META_FORMAT),
            harness: s.command.display(),
            args: s.command.as_args().to_vec(),
            cwd: Some(workspace.path.clone()),
            created: s.created,
            // Status is derived from the event log, not persisted — the log
            // is the source of truth for lifecycle state.
            status: None,
            exit_code: None,
            name: s.name.lock().unwrap().clone(),
            workspace: Some(workspace),
        }
    }
}
