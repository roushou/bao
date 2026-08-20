//! Resilience slice: a session survives a daemon restart — listed as
//! `interrupted`, history replayable, removable via `bao rm`.

mod common;

use std::time::Duration;

use bao_core::types::{Command, LaunchRequest, TerminalSize};
use bao_wire::Conn;

#[tokio::test]
async fn session_survives_daemon_restart() {
    let home = common::unique_home("resilience");
    let port = common::free_port();
    let addr = common::addr(port);

    // Phase 1: launch a session and interact with it.
    let mut host = common::start_host(&home, port).await;
    let mut a = Conn::connect(&addr).await.unwrap();
    let cmd = "bash -c 'echo READY; while read -r line; do echo \"got:$line\"; done'";
    let scratch = common::scratch_dir("resilience");
    let meta = a
        .launch(LaunchRequest {
            command: Some(Command::parse(cmd).unwrap()),
            dir: Some(scratch),
            name: None,
            size: TerminalSize {
                cols: 120,
                rows: 40,
            },
            sandbox: bao_core::sandbox::SandboxSpec::default(),
        })
        .await
        .unwrap();
    let sid = meta.id;
    common::wait_for_output(&mut a, "READY", "session starts").await;
    a.input(&sid, b"persist me\r").await.unwrap();
    common::wait_for_output(&mut a, "got:persist me", "session answers").await;
    drop(a);

    // Phase 2: kill the daemon hard (simulated crash — no graceful shutdown).
    host.kill().await.unwrap();
    let _ = host.wait().await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Phase 3: restart on the same home; the session must come back, honestly
    // marked interrupted, with its history replayable.
    let mut host2 = common::start_host(&home, port).await;
    let mut b = Conn::connect(&addr).await.unwrap();
    let sessions = b.list().await.unwrap();
    assert_eq!(sessions.len(), 1, "expected exactly the one session back");
    assert_eq!(sessions[0].id, sid);
    assert_eq!(sessions[0].status, bao_core::types::Status::Interrupted);

    // Attach: replay must include the interaction from before the crash.
    let mut c = Conn::connect(&addr).await.unwrap();
    let (attached, _seq, screen) = c.attach(&sid).await.unwrap();
    assert_eq!(
        attached.status,
        bao_core::types::Status::Interrupted,
        "attach metadata must report interrupted"
    );
    let text = String::from_utf8_lossy(&screen);
    assert!(text.contains("READY"), "screen missed early output");
    assert!(
        text.contains("got:persist me"),
        "screen missed pre-crash interaction: {text}"
    );

    // Input to an interrupted session must be refused honestly.
    let err = c.input(&sid, b"nope\r").await;
    assert!(err.is_err(), "input to an interrupted session must fail");

    // Phase 4: rm forgets the session and its files.
    c.rm(&sid).await.unwrap();
    let list2 = b.list().await.unwrap();
    assert!(list2.is_empty(), "rm must remove the session");
    assert!(
        !home.join("sessions").join(sid.as_str()).exists(),
        "rm must delete session files"
    );

    host2.kill().await.unwrap();
    let _ = std::fs::remove_dir_all(&home);
}
