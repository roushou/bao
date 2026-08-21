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

/// Append one event to a session's `events.log` as JSONL, checksummed. `kind`
/// is always written explicitly for new entries; legacy readers treat a
/// missing `kind` as `output`. Lines without a `crc` are trusted (they
/// predate checksums); every new line carries one.
pub(crate) fn persist_event(data_dir: &Path, seq: u64, ts: u64, kind: &EventKind) {
    let file = data_dir.join("events.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)
    {
        let mut line = match kind {
            EventKind::Output(bytes) => {
                serde_json::json!({"seq": seq, "ts": ts, "kind": "output", "data": b64e(bytes)})
            }
            EventKind::Status(status) => {
                serde_json::json!({"seq": seq, "ts": ts, "kind": "status", "status": status})
            }
        };
        let crc = line_crc(&line);
        line["crc"] = serde_json::json!(crc);
        let mut line = line.to_string();
        line.push('\n');
        let _ = f.write_all(line.as_bytes());
    }
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

/// Does this parsed line's checksum hold? Lines without a `crc` (written
/// before checksums existed) pass — legacy lines are trusted as before.
pub(crate) fn line_checksum_ok(v: &serde_json::Value) -> bool {
    match v.get("crc").and_then(|c| c.as_u64()) {
        Some(expected) => u64::from(line_crc(v)) == expected,
        None => true,
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

        // Legacy lines (no crc) pass — they predate checksums.
        let legacy = serde_json::json!({"seq": 1, "ts": 1, "data": "aGk="});
        assert!(line_checksum_ok(&legacy));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
