//! Sessions: one running session process + its terminal event log.
//!
//! Sessions persist `meta.json` + an append-only `events.log`; on startup
//! [`Session::restore_all`] rebuilds them from disk — honestly marked
//! [`Status::Interrupted`] when the process is gone.

mod log;
mod manager;
mod process;
mod store;

pub use manager::{Manager, StateEvent};
pub use process::Session;
pub(crate) use store::SessionStore;

use std::{
    collections::{HashMap, VecDeque},
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, watch};

use bao_core::{
    alert::{Alert, AlertInput, idle_secs},
    event::{EventKind, SessionEvent, fold_status},
    lifecycle::{LifecycleEvent, Transition},
    sandbox::{SandboxKind, SandboxSpec, Workspace},
    types::{Clock, Command, SessionId, SessionMeta, Status, TerminalSize, now_ms},
};

use crate::{
    error::Error,
    sandbox::{Sandbox, WorkspaceStore},
    screen as vt_screen,
};

/// Everything needed to launch one session's process (the spawn parameter
/// object — no more seven-argument constructors).
#[derive(Debug, Clone)]
pub struct SessionSpec {
    pub id: SessionId,
    pub name: Option<String>,
    pub command: Command,
    pub workspace: Workspace,
    pub size: TerminalSize,
    /// Time source — system in production; tests inject a fake clock to
    /// drive idle-alert derivation.
    pub clock: Clock,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::home::Home;
    use bao_core::types::TermStrExt;

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bao-core-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_session(dir: &Path, id: &str, outputs: &[(&str, &str)]) {
        let sess_dir = dir.join(id);
        std::fs::create_dir_all(&sess_dir).unwrap();
        std::fs::write(
            sess_dir.join("meta.json"),
            serde_json::json!({
                "id": id,
                "command": "bash fixture-session.sh",
                "cwd": "/tmp",
                "created": 1000,
            })
            .to_string(),
        )
        .unwrap();
        for (seq, text) in outputs {
            let seq: u64 = seq.parse().unwrap();
            super::log::persist_event(
                &sess_dir,
                seq,
                seq,
                &EventKind::Output(text.as_bytes().to_vec()),
            );
        }
    }

    #[test]
    fn fold_derives_status_and_applies_honesty() {
        use EventKind::{Output, Status as Ev};
        // Status events are authoritative; fold from Preparing.
        let full = VecDeque::from(vec![
            (1u64, Ev(Status::Starting)),
            (2u64, Output(b"hi".to_vec())),
            (3u64, Ev(Status::Exited(Some(3)))),
        ]);
        assert_eq!(fold_status(&full), Status::Exited(Some(3)));

        // Booting but no exit: live on restore → Interrupted.
        let booting = VecDeque::from(vec![
            (1u64, Ev(Status::Starting)),
            (2u64, Output(b"hi".to_vec())),
        ]);
        assert_eq!(fold_status(&booting), Status::Interrupted);

        // No status events at all: Preparing seed → Interrupted.
        let bare = VecDeque::from(vec![(1u64, Output(b"hi".to_vec()))]);
        assert_eq!(fold_status(&bare), Status::Interrupted);
    }

    #[test]
    fn restore_marks_was_running_as_interrupted() {
        let dir = temp_root("interrupted");
        write_session(&dir, "abc12345", &[("1", "hello\r\n"), ("2", "world\r\n")]);
        let store = SessionStore::new(dir.clone());
        let restored = Session::restore_all(&store).unwrap();
        assert_eq!(restored.len(), 1);
        let s = &restored[0];
        assert_eq!(s.id.as_str(), "abc12345");
        assert_eq!(s.status(), Status::Interrupted);
        assert_eq!(s.command.display(), "bash fixture-session.sh");
        let (snap, last) = s.snapshot_and_last(0);
        assert_eq!(last, 2);
        assert_eq!(snap.len(), 2);
        match &snap[0].kind {
            EventKind::Output(b) => assert_eq!(b, b"hello\r\n"),
            other => panic!("expected output, got {other:?}"),
        }
        match &snap[1].kind {
            EventKind::Output(b) => assert_eq!(b, b"world\r\n"),
            other => panic!("expected output, got {other:?}"),
        }
        assert!(s.input(b"x").is_err());
        assert!(s.resize(TerminalSize { cols: 80, rows: 24 }).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn restore_skips_garbage_and_missing_meta() {
        let dir = temp_root("garbage");
        std::fs::create_dir_all(dir.join("nometa")).unwrap();
        std::fs::write(dir.join("notadir.json"), "{}").unwrap();
        let store = SessionStore::new(dir.clone());
        let restored = Session::restore_all(&store).unwrap();
        assert!(restored.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn load_log_counts_checksum_corruption() {
        use std::str::FromStr;

        let dir = temp_root("crc");
        let store = SessionStore::new(dir.clone());
        let sess_dir = dir.join("abc12345");
        std::fs::create_dir_all(&sess_dir).unwrap();
        super::log::persist_event(
            &sess_dir,
            1,
            1000,
            &EventKind::Output(b"hello\r\n".to_vec()),
        );
        super::log::persist_event(
            &sess_dir,
            2,
            2000,
            &EventKind::Output(b"world\r\n".to_vec()),
        );

        // Tamper with the second line's payload: valid JSON, wrong bytes.
        let path = sess_dir.join("events.log");
        let raw = std::fs::read_to_string(&path).unwrap();
        let mut lines: Vec<String> = raw.lines().map(String::from).collect();
        let mut v: serde_json::Value = serde_json::from_str(&lines[1]).unwrap();
        v["data"] = serde_json::json!("AAAAAAAA");
        lines[1] = v.to_string();
        std::fs::write(&path, lines.join("\n") + "\n").unwrap();

        let loaded = store.load_log(&SessionId::from_str("abc12345").unwrap());
        assert_eq!(loaded.log.len(), 1, "the good line survives");
        assert_eq!(loaded.corrupt_lines, 1, "the tampered line is counted");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn restore_salvages_corrupt_meta_keeping_log() {
        let dir = temp_root("salvage");
        // Truncated (corrupt) meta.json + a valid log — the crash-mid-write
        // case the store must survive without losing the session.
        let sess_dir = dir.join("damaged01");
        std::fs::create_dir_all(&sess_dir).unwrap();
        std::fs::write(
            sess_dir.join("meta.json"),
            "{\"id\": \"damaged01\", \"command\":",
        )
        .unwrap();
        super::log::persist_event(
            &sess_dir,
            1,
            1000,
            &EventKind::Output(b"hello\r\n".to_vec()),
        );
        super::log::persist_event(
            &sess_dir,
            2,
            2000,
            &EventKind::Output(b"world\r\n".to_vec()),
        );

        let store = SessionStore::new(dir.clone());
        let restored = Session::restore_all(&store).unwrap();
        assert_eq!(restored.len(), 1, "damaged session must not be dropped");
        let s = &restored[0];
        assert_eq!(s.status(), Status::Damaged);

        // History survives for viewing (and eventual removal).
        let (snap, last) = s.snapshot_and_last(0);
        assert_eq!(last, 2);
        assert_eq!(snap.len(), 2);
        assert!(s.meta().last_output.contains("world"), "snippet from log");
        assert_eq!(s.meta().alert, Some(Alert::Damaged));

        // No known command — resume must fail honestly.
        assert!(
            s.resume(
                &Command::parse("bash demo.sh").unwrap(),
                TerminalSize { cols: 80, rows: 24 },
            )
            .is_err()
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn restore_marks_newer_format_as_damaged() {
        let dir = temp_root("newer");
        let sess_dir = dir.join("newer01");
        std::fs::create_dir_all(&sess_dir).unwrap();
        std::fs::write(
            sess_dir.join("meta.json"),
            serde_json::json!({"format": 99, "id": "newer01"}).to_string(),
        )
        .unwrap();

        let store = SessionStore::new(dir.clone());
        let restored = Session::restore_all(&store).unwrap();
        assert_eq!(
            restored.len(),
            1,
            "newer-format session must not be dropped"
        );
        assert_eq!(restored[0].status(), Status::Damaged);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn meta_reports_last_activity_and_output_snippet() {
        let dir = temp_root("meta");
        write_session(
            &dir.join("sessions"),
            "feed0002",
            &[
                ("1", "hello\r\n"),
                ("2", "\u{1b}[31mred text\u{1b}[0m and\nmore\r\n"),
            ],
        );
        let m = Manager::open(&Home::new(&dir)).unwrap();
        let sess = m.resolve("feed0002").unwrap();
        let meta = sess.meta();
        assert!(meta.last_activity >= 2, "last_activity from log ts");
        assert!(
            meta.last_output.contains("red text"),
            "ANSI must be stripped: {:?}",
            meta.last_output
        );
        assert!(meta.last_output.contains("and more"), "newlines collapsed");
        assert!(
            !meta.last_output.contains('\u{1b}'),
            "no escapes in snippet"
        );
        assert!(meta.last_output.chars().count() <= 65, "snippet truncated");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn meta_reports_live_last_activity() {
        use std::time::Duration;

        let dir = temp_root("meta-live");
        let m = Manager::open(&Home::new(&dir)).unwrap();
        let store = WorkspaceStore::new(dir.join("workspaces"));
        let sandbox = Sandbox::create(
            &store,
            &SessionId::from_str("live0001").unwrap(),
            &dir,
            &SandboxSpec {
                isolation: SandboxKind::InPlace,
            },
        )
        .unwrap();
        let command = Command::parse("bash -c 'echo PING_LIVE'").unwrap();
        let sess = m
            .create(
                &command,
                &sandbox.workspace.path,
                TerminalSize { cols: 80, rows: 24 },
                None,
                &SandboxSpec {
                    isolation: SandboxKind::InPlace,
                },
            )
            .unwrap();
        let mut rx = sess.subscribe();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while !sess.meta().last_output.contains("PING_LIVE") {
            assert!(
                tokio::time::Instant::now() < deadline,
                "session never output"
            );
            let _ = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
        }
        let meta = sess.meta();
        assert!(meta.last_activity > 0);
        assert_eq!(meta.last_output, "PING_LIVE");
        sess.kill().unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn idle_alert_derives_from_injected_clock() {
        use std::{
            sync::atomic::{AtomicU64, Ordering},
            time::Duration,
        };

        // A fake clock drives time-based derivation — no real waiting.
        static FAKE_NOW: AtomicU64 = AtomicU64::new(1_000_000);
        let clock = Clock(|| FAKE_NOW.load(Ordering::SeqCst));

        let dir = temp_root("clock");
        let store = SessionStore::new(dir.clone());
        let spec = SessionSpec {
            id: SessionId::from_str("clock0001").unwrap(),
            name: None,
            command: Command::parse("bash -c 'echo CLOCK_UP; sleep 30'").unwrap(),
            workspace: Workspace {
                kind: SandboxKind::InPlace,
                repo: None,
                branch: None,
                path: dir.clone(),
            },
            size: TerminalSize { cols: 80, rows: 24 },
            clock,
        };
        let sess = Session::spawn(&spec, &store).unwrap();
        let mut rx = sess.subscribe();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while !sess.meta().last_output.contains("CLOCK_UP") {
            assert!(
                tokio::time::Instant::now() < deadline,
                "session never output"
            );
            let _ = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
        }
        assert_eq!(sess.meta().alert, None, "fresh and active");

        // Advance the injected clock past the idle threshold — the derived
        // alert flips without any real time passing.
        FAKE_NOW.store(
            1_000_000 + (bao_core::alert::IDLE_ALERT_SECS + 5) * 1000,
            Ordering::SeqCst,
        );
        sess.publish_state();
        assert!(
            matches!(sess.meta().alert, Some(Alert::Idle(_))),
            "idle after the threshold on the injected clock"
        );

        sess.kill().unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn resume_relaunches_an_interrupted_session() {
        use std::time::Duration;

        let home = temp_root("resume");
        write_session(&home.join("sessions"), "resume0001", &[("1", "hello\r\n")]);
        let m = Manager::open(&Home::new(&home)).unwrap();
        let sess = m.resolve("resume0001").unwrap();
        assert_eq!(sess.status(), Status::Interrupted);
        assert!(
            sess.input(b"x").is_err(),
            "input must fail while interrupted"
        );

        let mut rx = sess.subscribe();
        let command = Command::parse(
            "bash -c 'echo RESUMED; while read -r line; do echo \"got:$line\"; done'",
        )
        .unwrap();
        sess.resume(
            &command,
            TerminalSize {
                cols: 120,
                rows: 40,
            },
        )
        .unwrap();
        // Resume relaunches into `Starting` — the process is up, awaiting its
        // first output (the honest boot state).
        assert_eq!(sess.status(), Status::Starting);

        let mut seen = String::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while !seen.contains("RESUMED") {
            assert!(
                tokio::time::Instant::now() < deadline,
                "timeout; saw {seen:?}"
            );
            match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
                Ok(Ok(ev)) => {
                    if let EventKind::Output(b) = &ev.kind {
                        seen.push_str(&String::from_utf8_lossy(b));
                    }
                }
                Ok(Err(e)) => panic!("broadcast error: {e:?}"),
                Err(_) => panic!("timeout; saw {seen:?}"),
            }
        }
        assert_eq!(
            sess.status(),
            Status::Running,
            "first output flips to running"
        );

        sess.input(b"ping\r").unwrap();
        let mut seen2 = String::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while !seen2.contains("got:ping") {
            assert!(
                tokio::time::Instant::now() < deadline,
                "timeout; saw {seen2:?}"
            );
            match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
                Ok(Ok(ev)) => {
                    if let EventKind::Output(b) = &ev.kind {
                        seen2.push_str(&String::from_utf8_lossy(b));
                    }
                }
                Ok(Err(e)) => panic!("broadcast error: {e:?}"),
                Err(_) => panic!("timeout; saw {seen2:?}"),
            }
        }

        let (snap, last) = sess.snapshot_and_last(0);
        assert!(
            snap.len() >= 4,
            "expected restored + resumed entries: {snap:?}"
        );
        assert!(last > 2, "seq must continue past the restored entry");
        assert!(
            snap.iter()
                .any(|e| matches!(&e.kind, EventKind::Output(b) if b == b"hello\r\n")),
            "restored history must remain in the log"
        );

        assert!(
            sess.resume(&command, TerminalSize { cols: 80, rows: 24 })
                .is_err()
        );
        sess.kill().unwrap();
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[tokio::test]
    async fn resize_keeps_pty_and_snapshot_parser_in_sync() {
        let dir = temp_root("resize");
        let store = SessionStore::new(dir.clone());
        let spec = SessionSpec {
            id: SessionId::from_str("resize0001").unwrap(),
            name: None,
            command: Command::parse("bash -c 'sleep 30'").unwrap(),
            workspace: Workspace {
                kind: SandboxKind::InPlace,
                repo: None,
                branch: None,
                path: dir.clone(),
            },
            size: TerminalSize { cols: 80, rows: 24 },
            clock: Clock::system(),
        };
        let sess = Session::spawn(&spec, &store).unwrap();
        assert_eq!(sess.screen.lock().unwrap().screen().size(), (24, 80));

        // The single size-mutation point: the snapshot parser must track the
        // PTY on every resize.
        sess.resize(TerminalSize {
            cols: 100,
            rows: 40,
        })
        .unwrap();
        assert_eq!(sess.screen.lock().unwrap().screen().size(), (40, 100));

        sess.kill().unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn resume_brings_snapshot_parser_to_the_resumed_size() {
        let home = temp_root("resume-size");
        write_session(&home.join("sessions"), "resz0001", &[("1", "hi\r\n")]);
        let m = Manager::open(&Home::new(&home)).unwrap();
        let sess = m.resolve("resz0001").unwrap();
        assert_eq!(sess.status(), Status::Interrupted);
        // Restored sessions use the default restore size until resumed.
        assert_eq!(sess.screen.lock().unwrap().screen().size(), (40, 120));

        let command = Command::parse("bash -c 'sleep 30'").unwrap();
        sess.resume(&command, TerminalSize { cols: 90, rows: 30 })
            .unwrap();
        assert_eq!(sess.screen.lock().unwrap().screen().size(), (30, 90));

        sess.kill().unwrap();
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn manager_open_loads_and_remove_forgets() {
        let home = temp_root("manager");
        write_session(&home.join("sessions"), "feed0001", &[("1", "hi\r\n")]);
        let m = Manager::open(&Home::new(&home)).unwrap();
        assert_eq!(m.list().len(), 1);
        assert_eq!(m.list()[0].status(), Status::Interrupted);
        m.remove("feed0001").unwrap();
        assert!(m.list().is_empty());
        assert!(!home.join("sessions").join("feed0001").exists());
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn strip_ansi_removes_csi_and_osc() {
        assert_eq!("\u{1b}[31mred\u{1b}[0m".strip_ansi(), "red");
        assert_eq!("\u{1b}]0;title\u{07}body".strip_ansi(), "body");
        assert_eq!("plain".strip_ansi(), "plain");
        assert_eq!("a\u{1b}[1;2;3Hb".strip_ansi(), "ab");
    }

    #[test]
    fn agent_command_parse_round_trips() {
        let c = Command::parse("bash -c 'echo READY; do echo \"got:$x\"; done'").unwrap();
        assert_eq!(
            c.as_args(),
            &[
                "bash".to_string(),
                "-c".to_string(),
                "echo READY; do echo \"got:$x\"; done".to_string()
            ]
        );
        assert!(Command::parse("   ").is_err());
    }

    #[tokio::test]
    async fn set_waiting_flows_into_meta_and_state_bus() {
        let dir = temp_root("waiting");
        let m = Manager::open(&Home::new(&dir)).unwrap();
        let store = WorkspaceStore::new(dir.join("workspaces"));
        let sandbox = Sandbox::create(
            &store,
            &SessionId::from_str("wait0001").unwrap(),
            &dir,
            &SandboxSpec {
                isolation: SandboxKind::InPlace,
            },
        )
        .unwrap();
        let command = Command::parse("bash -c 'echo WAIT_UP; sleep 30'").unwrap();
        let sess = m
            .create(
                &command,
                &sandbox.workspace.path,
                TerminalSize { cols: 80, rows: 24 },
                None,
                &SandboxSpec {
                    isolation: SandboxKind::InPlace,
                },
            )
            .unwrap();
        assert_eq!(sess.meta().waiting_for_input, None, "honest by default");

        // The daemon stores what the harness reports — the state bus carries
        // it to every watcher.
        let mut bus = m.subscribe_state();
        sess.set_waiting(Some(true));
        assert_eq!(sess.meta().waiting_for_input, Some(true));
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match bus.recv().await {
                Ok(StateEvent::Snapshot(meta))
                    if meta.id == sess.id && meta.waiting_for_input == Some(true) =>
                {
                    break;
                }
                Ok(_) => {}
                Err(_) => panic!("state bus closed"),
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "waiting state never streamed"
            );
        }

        // Clearing works too.
        sess.set_waiting(None);
        assert_eq!(sess.meta().waiting_for_input, None);
        sess.kill().unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn failed_launch_compensates_and_broadcasts_gone() {
        let dir = temp_root("failed-launch");
        let m = Manager::open_with(
            &Home::new(&dir),
            Arc::new(crate::sandbox::fake::FakeSandboxFactory {
                kind: SandboxKind::Worktree,
                fail: true,
            }),
        )
        .unwrap();
        let mut bus = m.subscribe_state();

        // The injected sandbox fails deterministically — no git, no disk —
        // and the saga must compensate and forget.
        let command = Command::parse("bash -c 'echo hi'").unwrap();
        let spec = SandboxSpec {
            isolation: SandboxKind::Worktree,
        };
        let err = match m.create(
            &command,
            &dir,
            TerminalSize { cols: 80, rows: 24 },
            None,
            &spec,
        ) {
            Err(e) => e,
            Ok(_) => panic!("expected launch to fail"),
        };
        assert!(
            matches!(err, Error::SandboxUnavailable(SandboxKind::Worktree)),
            "sandbox step must fail: {err:?}"
        );

        // No zombie session left behind.
        assert!(m.list().is_empty(), "failed launch must not linger");

        // Watchers learn the session is gone, with a reason.
        let mut saw_gone = false;
        while let Ok(ev) = bus.try_recv() {
            if matches!(
                ev,
                StateEvent::Gone {
                    reason: Some(_),
                    ..
                }
            ) {
                saw_gone = true;
            }
        }
        assert!(saw_gone, "failed launch must broadcast Gone");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
