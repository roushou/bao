//! The event log's on-disk helpers: JSONL encoding, base64, and the
//! output snippet.

use std::{io::Write, path::Path};

use bao_core::{error::Error, event::EventKind, types::TermStrExt};

/// Max entries kept in the in-memory log (ring buffer).
pub(crate) const LOG_CAP: usize = 20_000;

/// The session's most recent output, ANSI-stripped and collapsed to one line.
const SNIPPET_MAX: usize = 64;

pub(crate) fn b64e(bytes: &[u8]) -> String {
    use base64::{Engine, engine::general_purpose::STANDARD as B64};
    B64.encode(bytes)
}

pub(crate) fn b64d(s: &str) -> Result<Vec<u8>, Error> {
    use base64::{Engine, engine::general_purpose::STANDARD as B64};
    Ok(B64.decode(s)?)
}

/// Append one event to a session's `events.log` as JSONL. `kind` is always
/// written explicitly for new entries; legacy readers treat a missing `kind`
/// as `output`.
pub(crate) fn persist_event(data_dir: &Path, seq: u64, ts: u64, kind: &EventKind) {
    let file = data_dir.join("events.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)
    {
        let line = match kind {
            EventKind::Output(bytes) => {
                serde_json::json!({"seq": seq, "ts": ts, "kind": "output", "data": b64e(bytes)})
            }
            EventKind::Status(status) => {
                serde_json::json!({"seq": seq, "ts": ts, "kind": "status", "status": status})
            }
        };
        let mut line = line.to_string();
        line.push('\n');
        let _ = f.write_all(line.as_bytes());
    }
}

pub(crate) fn output_snippet(bytes: &[u8]) -> String {
    let raw = String::from_utf8_lossy(bytes);
    let clean = raw.strip_ansi();
    let one_line = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    one_line.as_str().truncate(SNIPPET_MAX)
}
