//! Session meta carries the facts the overview needs —
//! last activity and a last-output snippet (ANSI-stripped).

mod common;

use std::time::Duration;

use bao_client::Conn;
use bao_core::types::{Command, TerminalSize};

#[tokio::test]
async fn list_meta_carries_activity_facts() {
    let home = common::unique_home("session-meta");
    let port = common::free_port();
    let addr = common::addr(port);
    let scratch = common::scratch_dir("session-meta");
    let mut host = common::start_host(&home, port).await;

    let mut a = Conn::connect(&addr).await.unwrap();
    let meta = a
        .launch(
            Some(
                Command::parse(
                    "bash -c 'echo HELLO_SESSION; while read -r line; do echo \"got:$line\"; done'",
                )
                .unwrap(),
            ),
            Some(scratch),
            Some("watcher".to_string()),
            TerminalSize {
                cols: 120,
                rows: 40,
            },
            common::in_place(),
        )
        .await
        .unwrap();
    let sid = meta.id.clone();
    common::wait_for_output(&mut a, "HELLO_SESSION", "session outputs").await;

    let sessions = a.list().await.unwrap();
    let session = sessions
        .iter()
        .find(|s| s.id == sid)
        .expect("session in list");

    let last_activity = session.last_activity;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    assert!(
        now.saturating_sub(last_activity) < 30_000,
        "last_activity should be recent (within 30s)"
    );
    assert!(
        session.last_output.contains("HELLO_SESSION"),
        "last_output should include the session's output, got {:?}",
        session.last_output
    );
    assert!(
        !session.last_output.contains('\u{1b}'),
        "snippet must be ANSI-stripped"
    );
    // Pole B contract: the daemon derives alert; views never compute it.
    assert_eq!(
        session.alert, None,
        "a fresh active session must not be flagged"
    );
    assert!(
        !session.host.as_str().is_empty(),
        "host fact must be present"
    );
    assert_eq!(
        session.waiting_for_input, None,
        "waiting_for_input stays honest: unknown until an adapter hook says so"
    );

    a.rm(&sid).await.unwrap();
    host.kill().await.unwrap();
    let _ = tokio::time::timeout(Duration::from_secs(5), host.wait()).await;
    let _ = std::fs::remove_dir_all(&home);
}
