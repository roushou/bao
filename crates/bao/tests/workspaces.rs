//! Workspaces: registration and targeted launch, end to end over the wire.
//!
//! A session aimed at a workspace runs at the workspace's root — wherever
//! the client happens to be. The registry persists on the host, so a
//! restarted daemon still resolves the alias.

mod common;

use std::time::Duration;

use bao_client::{Conn, Error};
use bao_core::{
    sandbox::{SandboxKind, SandboxSpec},
    types::{Command, TerminalSize},
};

fn in_place() -> SandboxSpec {
    SandboxSpec {
        isolation: SandboxKind::InPlace,
    }
}

const SIZE: TerminalSize = TerminalSize {
    cols: 120,
    rows: 40,
};

#[tokio::test]
async fn workspace_targeted_launch_runs_at_workspace_root() {
    let home = common::unique_home("ws-e2e");
    let port = common::free_port();
    let addr = common::addr(port);
    let scratch = common::scratch_dir("ws-e2e");
    std::fs::create_dir_all(scratch.join("app")).unwrap();

    // Phase 1: register, target a launch, refuse an unknown alias.
    let mut host = common::start_host(&home, port).await;
    let first = tokio::time::timeout(Duration::from_secs(30), async {
        let mut conn = Conn::connect(&addr).await.unwrap();

        // Register: alias → root (the daemon canonicalizes the path).
        let ws = conn.workspace_add("app", &scratch.join("app")).await.unwrap();
        assert_eq!(ws.alias, "app");
        assert!(ws.root.is_absolute());

        // A launch aimed at the workspace runs at its root — no dir sent,
        // no client-side knowledge of paths required.
        let meta = conn
            .launch_in(
                "app",
                Some(Command::parse("bash -c 'sleep 30'").unwrap()),
                None,
                SIZE,
                in_place(),
            )
            .await
            .unwrap();
        assert!(
            meta.working_copy.path.starts_with(&ws.root),
            "session must run inside the workspace root: {}",
            meta.working_copy.path.display()
        );

        // An unknown alias is a typed refusal.
        let err = conn
            .launch_in("nope", None, None, SIZE, in_place())
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::Rpc(ref e) if matches!(e, bao_protocol::WireError::UnknownWorkspace { .. })),
            "expected UnknownWorkspace, got {err:?}"
        );

        conn.stop(&meta.id).await.unwrap();
    })
    .await;
    let _ = host.kill().await;
    first.expect("workspace phase 1 timed out");

    // Phase 2: a fresh daemon still resolves the alias — the registry is
    // host state, not client state.
    let mut host = common::start_host(&home, port).await;
    let second = tokio::time::timeout(Duration::from_secs(15), async {
        let mut conn = Conn::connect(&addr).await.unwrap();
        let list = conn.workspace_list().await.unwrap();
        assert_eq!(list.len(), 1, "registry must survive a restart");
        assert_eq!(list[0].alias, "app");
        assert!(list[0].root.starts_with(&scratch));

        // Forget; sessions already launched against it are untouched.
        conn.workspace_remove("app").await.unwrap();
        assert!(conn.workspace_list().await.unwrap().is_empty());
    })
    .await;
    let _ = host.kill().await;
    second.expect("workspace phase 2 timed out");
}
