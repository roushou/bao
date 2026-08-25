//! The `pi` harness adapter.
//!
//! pi auto-saves sessions as JSONL under `~/.pi/session/sessions/`, keyed by a
//! sanitized *canonical* absolute working-directory path:
//! `/Users/x/dev/bao` -> `--Users-x-dev-bao--`, with the newest
//! `<iso-timestamp>_<uuid>.jsonl` being the current session.
//!
//! Resume (verified by spike): `pi --session <file>` loads that exact session
//! and continues it — the conversation comes back intact.

use std::path::{Path, PathBuf};

use bao_core::{sandbox::WorkingCopy, types::Command};

use super::Harness;

/// The pi coding-session harness.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pi;

impl Harness for Pi {
    fn name(&self) -> &'static str {
        "pi"
    }

    fn matches(&self, command: &Command) -> bool {
        command.first() == "pi"
    }

    /// Extra args to relaunch pi with so it continues its conversation in
    /// `working_copy`. `None` when no session file exists (fresh launch, honest).
    fn resume_args(&self, working_copy: &WorkingCopy) -> Option<Vec<String>> {
        let file = self.latest_session_file(&working_copy.path)?;
        Some(vec![
            "--session".to_string(),
            file.to_string_lossy().into_owned(),
        ])
    }

    /// pi is waiting for the human exactly when its session log's last
    /// message is an assistant *text* response — a completed turn — rather
    /// than a tool call, a tool result, or the user's message. Derived from
    /// pi's own persisted session file, never guessed. `None` when there is
    /// no file to read (fresh launch, or not a pi-managed dir).
    fn waiting_for_input(&self, working_copy: &WorkingCopy) -> Option<bool> {
        let file = self.latest_session_file(&working_copy.path)?;
        let raw = std::fs::read_to_string(&file).ok()?;
        waiting_from_log(&raw)
    }
}

/// Decide the waiting state from the raw session log: `Some(true)` only when
/// the last message is an assistant *text* response (a completed turn).
/// Pure and unit-testable.
fn waiting_from_log(raw: &str) -> Option<bool> {
    // The last *message* line, skipping non-message lines (session_info,
    // model_change, …) and any unparseable tail.
    let last = raw.lines().rev().find_map(|l| {
        let v = serde_json::from_str::<serde_json::Value>(l.trim()).ok()?;
        if v.get("type").and_then(|t| t.as_str()) == Some("message") {
            Some(v)
        } else {
            None
        }
    })?;
    let msg = last.get("message")?;
    let role = msg.get("role")?.as_str()?;
    let last_block = msg
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|c| c.last())?;
    let block_type = last_block.get("type")?.as_str()?;
    Some(role == "assistant" && block_type == "text")
}

impl Pi {
    fn sessions_root(&self) -> Option<PathBuf> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .or_else(dirs::home_dir)?;
        Some(home.join(".pi").join("session").join("sessions"))
    }

    fn latest_session_file(&self, env_path: &Path) -> Option<PathBuf> {
        let root = self.sessions_root()?;
        // pi keys by the canonical path (macOS /tmp -> /private/tmp); match it.
        let canonical = env_path
            .canonicalize()
            .unwrap_or_else(|_| env_path.to_path_buf());
        let dir = root.join(self.sanitize(&canonical));
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
            .collect();
        // ISO-8601 timestamps sort lexicographically; newest is last.
        files.sort();
        files.last().cloned()
    }

    /// `/<a>/<b>/<c>` -> `--a-b-c--` (dots and spaces preserved, like pi).
    fn sanitize(&self, path: &Path) -> String {
        let raw = path.to_string_lossy();
        let stripped = raw.strip_prefix('/').unwrap_or(&raw);
        let inner = stripped.replace('/', "-");
        format!("--{inner}--")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_matches_pi_key_format() {
        let pi = Pi;
        assert_eq!(pi.sanitize(Path::new("/a/b/c")), "--a-b-c--");
        assert_eq!(
            pi.sanitize(Path::new("/Users/x/dev/bao")),
            "--Users-x-dev-bao--"
        );
        assert_eq!(pi.sanitize(Path::new("relative/path")), "--relative-path--");
    }

    #[test]
    fn resume_args_is_none_without_session_files() {
        let pi = Pi;
        // A path that can never have a pi session dir under the real HOME.
        let working_copy = WorkingCopy {
            path: "/nonexistent/bao-test-xyz".into(),
            ..WorkingCopy::default()
        };
        assert_eq!(pi.resume_args(&working_copy), None);
    }

    #[test]
    fn identifies_pi_by_command() {
        let pi = Pi;
        assert!(pi.matches(&Command::parse("pi --model x").unwrap()));
        assert!(!pi.matches(&Command::parse("claude").unwrap()));
    }

    fn msg(role: &str, last_block: &str) -> String {
        serde_json::json!({
            "type": "message",
            "message": {
                "role": role,
                "content": [{"type": "text", "text": "x"}, {"type": last_block, "x": 1}],
            }
        })
        .to_string()
    }

    #[test]
    fn waiting_is_proven_only_by_an_assistant_text_turn() {
        assert_eq!(waiting_from_log(&msg("assistant", "text")), Some(true));
        assert_eq!(waiting_from_log(&msg("assistant", "toolCall")), Some(false));
        assert_eq!(waiting_from_log(&msg("assistant", "thinking")), Some(false));
        assert_eq!(waiting_from_log(&msg("user", "text")), Some(false));
        assert_eq!(
            waiting_from_log(&serde_json::json!({"type":"message","message":{"role":"toolResult","content":[{"type":"text"}]}}).to_string()),
            Some(false)
        );
    }

    #[test]
    fn waiting_is_none_without_a_message_line() {
        assert_eq!(waiting_from_log(""), None);
        assert_eq!(
            waiting_from_log(&serde_json::json!({"type":"session_info"}).to_string()),
            None
        );
        // A trailing non-message line must not shadow the real last message.
        let log = format!(
            "{}\n{}",
            msg("assistant", "text"),
            serde_json::json!({"type":"session_info"})
        );
        assert_eq!(waiting_from_log(&log), Some(true));
    }
}
