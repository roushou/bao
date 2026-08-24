//! The session's event log: the in-memory ring + sequence + broadcast, and
//! its on-disk writer (JSONL encoding, base64, checksummed output snippet).

use std::{
    collections::VecDeque,
    io::Write,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use tokio::sync::{broadcast, mpsc, oneshot};

use bao_core::{
    event::{EventKind, SessionEvent},
    types::{SessionId, TermStrExt},
};

use crate::error::Error;

/// Max entries kept in the in-memory log (ring buffer).
pub(crate) const LOG_CAP: usize = 20_000;

/// Max pending log lines per session. The writer drains in microseconds;
/// overflow (a disk that can't keep up) drops the line — the in-memory log
/// still has it, and checksummed restore reflects what reached disk.
const LOG_WRITER_CAP: usize = 4096;

/// The session's most recent output, ANSI-stripped and collapsed to one line.
const SNIPPET_MAX: usize = 64;

/// How durable each append must be. Default: group commits, fsync at most
/// once per interval and always on flush.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Durability {
    /// fsync every line. Slowest, strongest. (Not selectable yet — lands
    /// with the daemon's durability config.)
    #[allow(dead_code)]
    PerEvent,
    /// Group commits: fsync at most every `interval`, always on flush/drop.
    Batched { interval: Duration },
}

impl Default for Durability {
    fn default() -> Self {
        Self::Batched {
            interval: Duration::from_secs(1),
        }
    }
}

enum LogCmd {
    Line(LogLine),
    Flush(oneshot::Sender<()>),
}

struct LogLine {
    seq: u64,
    ts: u64,
    kind: EventKind,
}

/// A session's on-disk event log, owned by one writer task. [`SessionLog`]
/// hands lines over a bounded channel (no blocking I/O on the pump); the
/// writer appends immediately and fsyncs per the durability policy. When the
/// sender is dropped (session dropped), the writer drains and fsyncs once
/// more.
struct LogWriter {
    tx: Option<mpsc::Sender<LogCmd>>,
}

impl LogWriter {
    fn spawn(data_dir: &Path, policy: Durability) -> Self {
        let (tx, mut rx) = mpsc::channel::<LogCmd>(LOG_WRITER_CAP);
        let file = data_dir.join("events.log");
        // Without a runtime (sync tests, early startup) the writer is a
        // no-op — nothing can append without the async pump anyway.
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return Self { tx: None };
        };
        // Detached: the task self-terminates when the channel closes (the
        // session is dropped), flushing once more on the way out.
        #[allow(clippy::let_underscore_future)]
        let _ = handle.spawn(async move {
            let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file)
            else {
                return; // can't write the log; the in-memory log still works
            };
            let mut last_sync = std::time::Instant::now();
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    LogCmd::Line(line) => {
                        write_line(&mut f, &line);
                        let due = match policy {
                            Durability::PerEvent => true,
                            Durability::Batched { interval } => last_sync.elapsed() >= interval,
                        };
                        if due {
                            let _ = f.sync_data();
                            last_sync = std::time::Instant::now();
                        }
                    }
                    LogCmd::Flush(ack) => {
                        let _ = f.sync_data();
                        let _ = ack.send(());
                    }
                }
            }
            let _ = f.sync_data(); // drain on shutdown
        });
        Self { tx: Some(tx) }
    }

    /// Queue one line. Bounded and non-blocking: a full queue drops the line
    /// rather than stalling the pump.
    fn append(&self, seq: u64, ts: u64, kind: &EventKind) {
        if let Some(tx) = &self.tx {
            let _ = tx.try_send(LogCmd::Line(LogLine {
                seq,
                ts,
                kind: kind.clone(),
            }));
        }
    }

    /// fsync everything queued so far and wait for it to land.
    async fn flush(&self) {
        let Some(tx) = &self.tx else {
            return;
        };
        let (ack, rx) = oneshot::channel();
        if tx.send(LogCmd::Flush(ack)).await.is_ok() {
            let _ = rx.await;
        }
    }
}

/// One session's event log: the in-memory ring buffer, the sequence counter,
/// the live-event broadcast, and the async writer — the four things every
/// append touches. `snapshot_after` is the attach-replay source.
pub(crate) struct SessionLog {
    id: SessionId,
    ring: Mutex<VecDeque<(u64, EventKind)>>,
    seq: AtomicU64,
    tx: broadcast::Sender<SessionEvent>,
    writer: Mutex<Option<Arc<LogWriter>>>,
}

impl SessionLog {
    /// A fresh log for a live session: empty ring, a writer at `data_dir`.
    pub(crate) fn new(id: &SessionId, data_dir: &Path, durability: Durability) -> Self {
        SessionLog {
            id: id.clone(),
            ring: Mutex::new(VecDeque::new()),
            seq: AtomicU64::new(0),
            tx: broadcast::channel(4096).0,
            writer: Mutex::new(Some(Arc::new(LogWriter::spawn(data_dir, durability)))),
        }
    }

    /// A log rebuilt from on-disk state (restore): ring from the file, the
    /// sequence continuing, no writer (no live process to append).
    pub(crate) fn restored(id: &SessionId, log: VecDeque<(u64, EventKind)>, last_seq: u64) -> Self {
        SessionLog {
            id: id.clone(),
            ring: Mutex::new(log),
            seq: AtomicU64::new(last_seq),
            tx: broadcast::channel(4096).0,
            writer: Mutex::new(None),
        }
    }

    /// The sequence the next append will be assigned.
    pub(crate) fn last_seq(&self) -> u64 {
        self.seq.load(Ordering::SeqCst)
    }

    /// Append an event: assign the seq, ring-buffer it, broadcast it, and
    /// hand it to the async writer. Returns the assigned seq.
    pub(crate) fn append(&self, kind: EventKind, ts: u64) -> u64 {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        {
            let mut ring = self.ring.lock().unwrap();
            if ring.len() >= LOG_CAP {
                ring.pop_front();
            }
            ring.push_back((seq, kind.clone()));
        }
        let _ = self.tx.send(SessionEvent {
            session: self.id.clone(),
            seq,
            ts,
            kind: kind.clone(),
        });
        if let Some(writer) = &*self.writer.lock().unwrap() {
            writer.append(seq, ts, &kind);
        }
        seq
    }

    /// Entries with seq > `after`, plus the latest seq, under one lock so
    /// attach-replay cannot race with new appends.
    pub(crate) fn snapshot_after(&self, after: u64) -> (Vec<SessionEvent>, u64) {
        let ring = self.ring.lock().unwrap();
        let out = ring
            .iter()
            .filter(|(s, _)| *s > after)
            .map(|(seq, kind)| SessionEvent {
                session: self.id.clone(),
                seq: *seq,
                ts: 0,
                kind: kind.clone(),
            })
            .collect();
        let last = ring.back().map(|(s, _)| *s).unwrap_or(0);
        (out, last)
    }

    /// Subscribe to the live event broadcast (every append).
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.tx.subscribe()
    }

    /// fsync everything queued so far and wait for it to land.
    pub(crate) async fn flush(&self) {
        let writer = self.writer.lock().unwrap().clone();
        if let Some(writer) = writer {
            writer.flush().await;
        }
    }
}

pub(crate) fn b64e(bytes: &[u8]) -> String {
    use base64::{Engine, engine::general_purpose::STANDARD as B64};
    B64.encode(bytes)
}

pub(crate) fn b64d(s: &str) -> Result<Vec<u8>, Error> {
    use base64::{Engine, engine::general_purpose::STANDARD as B64};
    Ok(B64.decode(s)?)
}

/// Append one event to a session's `events.log` as JSONL, checksummed — the
/// synchronous write path, used by tests and fixtures. Live sessions go
/// through [`LogWriter`] instead.
#[cfg(test)]
pub(crate) fn persist_event(data_dir: &Path, seq: u64, ts: u64, kind: &EventKind) {
    let file = data_dir.join("events.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)
    {
        write_line(
            &mut f,
            &LogLine {
                seq,
                ts,
                kind: kind.clone(),
            },
        );
    }
}

/// Write one checksummed line to the open log file.
fn write_line(f: &mut std::fs::File, line: &LogLine) {
    let mut v = match &line.kind {
        EventKind::Output(bytes) => serde_json::json!({
            "seq": line.seq,
            "ts": line.ts,
            "kind": "output",
            "data": b64e(bytes)
        }),
        EventKind::Status(status) => serde_json::json!({
            "seq": line.seq,
            "ts": line.ts,
            "kind": "status",
            "status": status
        }),
    };
    let crc = line_crc(&v);
    v["crc"] = serde_json::json!(crc);
    let mut s = v.to_string();
    s.push('\n');
    let _ = f.write_all(s.as_bytes());
}

/// The checksum covers the whole line except the `crc` field itself: the
/// parsed value (minus crc) is re-serialized and hashed, so any corruption
/// in seq/ts/kind/data that still parses as JSON is detected on restore.
pub(crate) fn line_crc(line: &serde_json::Value) -> u32 {
    let mut v = line.clone();
    if let Some(obj) = v.as_object_mut() {
        obj.remove("crc");
    }
    crc32fast::hash(v.to_string().as_bytes())
}

/// Does this parsed line's checksum hold? A line without a `crc` is corrupt.
pub(crate) fn line_checksum_ok(v: &serde_json::Value) -> bool {
    match v.get("crc").and_then(|c| c.as_u64()) {
        Some(expected) => u64::from(line_crc(v)) == expected,
        None => false,
    }
}

pub(crate) fn output_snippet(bytes: &[u8]) -> String {
    let raw = String::from_utf8_lossy(bytes);
    let clean = raw.strip_ansi();
    let one_line = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    one_line.as_str().truncate(SNIPPET_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_detects_tampering() {
        let dir = std::env::temp_dir().join(format!("bao-log-crc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        persist_event(&dir, 1, 1000, &EventKind::Output(b"hello\r\n".to_vec()));
        let raw = std::fs::read_to_string(dir.join("events.log")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(line_checksum_ok(&v), "an untouched line holds");

        // Valid JSON, wrong bytes: the payload must be caught.
        let mut v2 = v.clone();
        v2["data"] = serde_json::json!("AAAAAAAA");
        assert!(!line_checksum_ok(&v2), "tampered data is caught");

        // Tampered metadata is caught too — the crc covers the whole line.
        let mut v3 = v.clone();
        v3["seq"] = serde_json::json!(99);
        assert!(!line_checksum_ok(&v3), "tampered seq is caught");

        // A line without a crc is corrupt, not trusted.
        let missing = serde_json::json!({"seq": 1, "ts": 1, "data": "aGk="});
        assert!(!line_checksum_ok(&missing));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
