//! End-to-end tests for the walking skeleton and the state-first contract.
//!
//! Host setup and event draining come from `common`; only the
//! status-transition waiter is local to this file.

mod common;

use std::{str::FromStr, time::Duration};

use bao_client::{Conn, HostEvent};
use bao_core::types::{Command, Status, TerminalSize};

/// Drain events until the status transition to exited arrives, or timeout.
async fn wait_for_status(conn: &mut Conn, label: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        assert!(tokio::time::Instant::now() < deadline, "timeout: {label}");
        match conn.next_event().await {
            Some(HostEvent::Status {
                status: Status::Exited(_),
                ..
            }) => {
                return;
            }
            Some(HostEvent::Disconnected) | None => panic!("{label}: host disconnected"),
            Some(_) => {}
        }
    }
}

#[tokio::test]
async fn two_clients_share_one_live_session() {
    let home = common::unique_home("e2e");
    let port = common::free_port();
    let addr = common::addr(port);
    let scratch = common::scratch_dir("e2e");
    let mut host = common::start_host(&home, port).await;
    let test = async {
        // Client A launches the session (a bash loop that echoes lines back).
        let mut a = Conn::connect(&addr).await.unwrap();
        let cmd = "bash -c 'echo READY; while read -r line; do echo \"got:$line\"; done'";
        let meta = a
            .launch(
                Some(Command::parse(cmd).unwrap()),
                Some(scratch),
                None,
                None,
                None,
                TerminalSize {
                    cols: 120,
                    rows: 40,
                },
                common::in_place(),
            )
            .await
            .unwrap();
        let sid = meta.id.clone();

        // A sees the session come up.
        let a_out = common::wait_for_output(&mut a, "READY", "client A sees session start").await;
        assert!(a_out.contains("READY"), "A never saw READY: {a_out:?}");

        // Client B attaches with full replay and sees the same history.
        let mut b = Conn::connect(&addr).await.unwrap();
        let (b_meta, _seq, screen) = b.attach(&sid).await.unwrap();
        assert_eq!(b_meta.id, sid);
        let b_text = String::from_utf8_lossy(&screen);
        assert!(
            b_text.contains("READY"),
            "B's screen missed READY: {b_text}"
        );

        // A sends input; B must see the resulting output live.
        a.input(&sid, b"hi from A\r").await.unwrap();
        let b_live =
            common::wait_for_output(&mut b, "got:hi from A", "client B sees A's input echoed")
                .await;
        assert!(
            b_live.contains("got:hi from A"),
            "B missed live output: {b_live:?}"
        );

        // B sends input; A must see it too (both directions).
        b.input(&sid, b"hi from B\r").await.unwrap();
        let a_live =
            common::wait_for_output(&mut a, "got:hi from B", "client A sees B's input echoed")
                .await;
        assert!(
            a_live.contains("got:hi from B"),
            "A missed live output: {a_live:?}"
        );

        // Stop: both clients must observe the exited status.
        a.stop(&sid).await.unwrap();
        wait_for_status(&mut a, "client A sees exited").await;
        wait_for_status(&mut b, "client B sees exited").await;
    };
    tokio::time::timeout(Duration::from_secs(30), test)
        .await
        .expect("e2e test timed out");
    let _ = host.kill().await;
}

/// State-first contract (Pole B): a watcher connection receives the daemon's
/// derived pictures — including output snippets and status transitions — and
/// *never* a raw terminal byte. Terminal bytes are only for views that
/// explicitly attach.
#[tokio::test]
async fn watch_receives_state_and_never_bytes() {
    let home = common::unique_home("e2e-watch");
    let port = common::free_port();
    let addr = common::addr(port);
    let scratch = common::scratch_dir("e2e-watch");
    let mut host = common::start_host(&home, port).await;
    let test = async {
        let mut w = Conn::connect(&addr).await.unwrap();
        w.watch().await.unwrap();

        let mut a = Conn::connect(&addr).await.unwrap();
        let meta = a
            .launch(
                Some(Command::parse("bash -c 'echo STATE_HELLO; sleep 3'").unwrap()),
                Some(scratch),
                None,
                None,
                None,
                TerminalSize {
                    cols: 120,
                    rows: 40,
                },
                common::in_place(),
            )
            .await
            .unwrap();
        let sid = meta.id.clone();

        // The watcher sees the derived picture with the output snippet, and
        // the status transition when we stop the session.
        let mut saw_snippet = false;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            assert!(
                tokio::time::Instant::now() < deadline,
                "no state received (snippet={saw_snippet})"
            );
            match w.next_event().await {
                Some(HostEvent::State { meta, .. }) => {
                    assert_eq!(meta.id, sid, "state must name the session");
                    if meta.last_output.contains("STATE_HELLO") {
                        saw_snippet = true;
                    }
                    if saw_snippet && matches!(meta.status, Status::Exited(_)) {
                        break;
                    }
                    if saw_snippet && meta.status == Status::Running {
                        // Session is up and talking; stop it so the status
                        // transition arrives as a fact.
                        a.stop(&sid).await.unwrap();
                    }
                }
                Some(HostEvent::Output { .. }) => {
                    panic!("watcher must never receive terminal bytes")
                }
                Some(HostEvent::Disconnected) | None => panic!("watcher disconnected"),
                Some(_) => {}
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(30), test)
        .await
        .expect("state e2e timed out");
    let _ = host.kill().await;
}

/// Slice 6: rename — the daemon persists the new name and streams it.
#[tokio::test]
async fn rename_updates_session_name() {
    let home = common::unique_home("rename");
    let port = common::free_port();
    let addr = common::addr(port);
    let scratch = common::scratch_dir("rename");
    let mut host = common::start_host(&home, port).await;
    let test = async {
        let mut a = Conn::connect(&addr).await.unwrap();
        let meta = a
            .launch(
                Some(Command::parse("bash -c 'echo RENAME_ME; sleep 3'").unwrap()),
                Some(scratch),
                None,
                None,
                Some("old".to_string()),
                TerminalSize {
                    cols: 120,
                    rows: 40,
                },
                common::in_place(),
            )
            .await
            .unwrap();
        let sid = meta.id.clone();
        common::wait_for_output(&mut a, "RENAME_ME", "session up").await;

        a.rename(&sid, Some("renamed".to_string())).await.unwrap();
        let sessions = a.list().await.unwrap();
        let s = sessions.iter().find(|s| s.id == sid).unwrap();
        assert_eq!(s.name.as_deref(), Some("renamed"));

        // Clearing works too.
        a.rename(&sid, None).await.unwrap();
        let sessions = a.list().await.unwrap();
        let s = sessions.iter().find(|s| s.id == sid).unwrap();
        assert_eq!(s.name, None, "name cleared");

        a.rm(&sid).await.unwrap();
    };
    tokio::time::timeout(Duration::from_secs(30), test)
        .await
        .expect("rename test timed out");
    let _ = host.kill().await;
    let _ = std::fs::remove_dir_all(&home);
}

/// Slice 5: `bao launch --detach` works with no TTY and leaves a background
/// session the daemon keeps running.
#[tokio::test]
async fn launch_detach_works_without_a_terminal() {
    let home = common::unique_home("detach");
    let port = common::free_port();
    let scratch = common::scratch_dir("detach");
    let mut host = common::start_host(&home, port).await;

    let out = tokio::process::Command::new(env!("CARGO_BIN_EXE_bao"))
        .args([
            "launch",
            "--detach",
            "--cmd",
            "bash -c 'echo DETACHED; sleep 5'",
            "--name",
            "detached",
            "--dir",
            scratch.to_str().unwrap(),
            "--isolation",
            "inplace",
            "--port",
            &port.to_string(),
        ])
        .env("BAO_HOME", &home)
        .output()
        .await
        .unwrap();
    assert!(
        out.status.success(),
        "detached launch failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let sid = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert!(!sid.is_empty(), "detached launch must print the session id");

    let mut a = Conn::connect(&common::addr(port)).await.unwrap();
    // Launch is now backgrounded: the session starts `Preparing`/`Starting`
    // and reaches `Running` on first output — poll for that fact.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let s = loop {
        let sessions = a.list().await.unwrap();
        if let Some(s) = sessions
            .iter()
            .find(|s| s.id.as_str() == sid && s.name.as_deref() == Some("detached"))
            && s.status == bao_core::types::Status::Running
        {
            break s.clone();
        }
        assert!(
            std::time::Instant::now() < deadline,
            "detached session never reached running"
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    };
    assert_eq!(s.status, bao_core::types::Status::Running, "still running");

    a.rm(&bao_core::types::SessionId::from_str(&sid).unwrap())
        .await
        .unwrap();
    let _ = host.kill().await;
    let _ = std::fs::remove_dir_all(&home);
}

/// Wire hardening: the connect handshake learns the daemon's identity and
/// capabilities; errors come back typed and branchable.
#[tokio::test]
async fn handshake_reports_daemon_info_and_typed_errors() {
    let home = common::unique_home("info");
    let port = common::free_port();
    let addr = common::addr(port);
    let mut host = common::start_host(&home, port).await;
    let test = async {
        let conn = Conn::connect(&addr).await.unwrap();
        let info = conn.info();
        assert_eq!(
            info.protocol_version,
            bao_protocol::PROTOCOL_VERSION,
            "handshake version must match"
        );
        assert!(!info.host.as_str().is_empty());
        assert!(
            info.sandbox_backends
                .contains(&bao_core::sandbox::SandboxKind::Worktree),
            "the machine advertises what it can do"
        );

        // Typed wire errors: a client can branch on the kind.
        let mut a = Conn::connect(&addr).await.unwrap();
        let err = a
            .rename(
                &bao_core::types::SessionId::from_str("nosuch01").unwrap(),
                Some("x".to_string()),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                bao_client::Error::Rpc(bao_protocol::WireError::NotFound { .. })
            ),
            "rename of an unknown session must come back typed: {err}"
        );
    };
    tokio::time::timeout(Duration::from_secs(30), test)
        .await
        .expect("handshake test timed out");
    let _ = host.kill().await;
    let _ = std::fs::remove_dir_all(&home);
}

/// A backgrounded launch that fails its saga must roll back and signal `Gone`
/// on the attached stream (not just the the overview) — the `bao launch`
/// terminal must not sit empty forever.
#[tokio::test]
async fn failed_backgrounded_launch_signals_gone_on_attach_stream() {
    let home = common::unique_home("e2e-gone");
    let port = common::free_port();
    let addr = common::addr(port);
    let scratch = common::scratch_dir("e2e-gone");
    let mut host = common::start_host(&home, port).await;

    let mut a = Conn::connect(&addr).await.unwrap();
    // Request worktree isolation in a non-git dir: the sandbox step fails in
    // the background saga, which must roll the session back and signal Gone.
    let meta = a
        .launch(
            Some(Command::parse("bash -c 'echo hi'").unwrap()),
            Some(scratch),
            None,
            None,
            Some("doomed".to_string()),
            TerminalSize {
                cols: 120,
                rows: 40,
            },
            bao_core::sandbox::SandboxSpec {
                isolation: bao_core::sandbox::SandboxKind::Worktree,
            },
        )
        .await
        .unwrap();
    // The reply is the honest `Preparing` snapshot — the saga runs after.
    assert_eq!(meta.status, Status::Preparing);

    // The attached stream must carry the rollback as a `Gone` with a reason.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        assert!(
            tokio::time::Instant::now() < deadline,
            "no Gone arrived on the attach stream"
        );
        match a.next_event().await {
            Some(HostEvent::Gone {
                reason: Some(_), ..
            }) => break,
            Some(HostEvent::Disconnected) | None => panic!("host disconnected"),
            Some(_) => {}
        }
    }

    // The rolled-back session is gone from the list.
    let sessions = a.list().await.unwrap();
    assert!(
        !sessions.iter().any(|s| s.id == meta.id),
        "failed session must not linger"
    );

    host.kill().await.unwrap();
    let _ = std::fs::remove_dir_all(&home);
}
