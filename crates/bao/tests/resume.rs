//! Resume slice: an interrupted session (daemon crash) is relaunched with its
//! history intact and keeps accepting input; resuming a running session is
//! refused.

mod common;

use std::time::Duration;

use bao_client::Conn;
use bao_core::types::{Command, TerminalSize};

#[tokio::test]
async fn interrupted_session_can_be_resumed() {
    let home = common::unique_home("resume");
    let port = common::free_port();
    let addr = common::addr(port);
    let scratch = common::scratch_dir("resume");
    let mut host = common::start_host(&home, port).await;

    // Phase 1: launch and interact.
    let mut a = Conn::connect(&addr).await.unwrap();
    let cmd = "bash -c 'echo READY; while read -r line; do echo \"got:$line\"; done'";
    let meta = a
        .launch(
            Some(Command::parse(cmd).unwrap()),
            Some(scratch),
            None,
            None,
            Some("worker".to_string()),
            TerminalSize {
                cols: 120,
                rows: 40,
            },
            common::in_place(),
        )
        .await
        .unwrap();
    let sid = meta.id;
    common::wait_for_output(&mut a, "READY", "session starts").await;
    a.input(&sid, b"before restart\r").await.unwrap();
    common::wait_for_output(&mut a, "got:before restart", "pre-restart interaction").await;
    drop(a);

    // Phase 2: crash the daemon, restart.
    host.kill().await.unwrap();
    let _ = host.wait().await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let mut host2 = common::start_host(&home, port).await;

    // Phase 3: resume. The reply says running, and the subscription replays
    // the pre-crash history.
    let mut r = Conn::connect(&addr).await.unwrap();
    let meta = r
        .resume(
            &sid,
            TerminalSize {
                cols: 120,
                rows: 40,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        meta.status,
        bao_core::types::Status::Starting,
        "resume relaunches into the honest boot state (first output flips to running)"
    );
    let replay =
        common::wait_for_output(&mut r, "got:before restart", "history replays after resume").await;
    assert!(replay.contains("READY"), "replay missed early output");

    // Phase 4: the relaunched process answers new input.
    r.input(&sid, b"after resume\r").await.unwrap();
    let live = common::wait_for_output(&mut r, "got:after resume", "live after resume").await;
    assert!(live.contains("got:after resume"));

    // Phase 5: resuming a running session is refused.
    let err = r
        .resume(
            &sid,
            TerminalSize {
                cols: 120,
                rows: 40,
            },
        )
        .await;
    assert!(err.is_err(), "resume on a running session must fail");

    // Cleanup.
    r.rm(&sid).await.unwrap();
    host2.kill().await.unwrap();
    let _ = std::fs::remove_dir_all(&home);
}
