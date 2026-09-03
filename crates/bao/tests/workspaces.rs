//! Workspaces: registration and targeted launch, end to end over the wire.
//!
//! A session aimed at a workspace runs at the workspace's root — wherever
//! the client happens to be. The registry persists on the host, so a
//! restarted daemon still resolves the alias.

mod common;

use std::time::Duration;

use bao_client::{Conn, Error};
use bao_core::{
    registry::RegistryEntry,
    sandbox::{SandboxKind, SandboxSpec},
    types::{Command, TerminalSize},
};

fn in_place() -> SandboxSpec {
    SandboxSpec {
        isolation: Some(SandboxKind::InPlace),
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
        conn.registry_put(RegistryEntry::workspace("app", scratch.join("app")).unwrap())
            .await
            .unwrap();
        let listed = conn.registry_list().await.unwrap();
        let ws = listed.iter().find(|e| e.alias == "app").unwrap();
        assert!(ws.root().expect("workspace entry").is_absolute());

        // A launch aimed at the workspace runs at its root — no dir sent,
        // no client-side knowledge of paths required.
        let meta = conn
            .launch(
                None,
                None,
                Some("app"),
                None,
                None,
                SIZE,
                in_place(),
            )
            .await
            .unwrap();
        assert!(
            meta.working_copy.path.starts_with(ws.root().unwrap()),
            "session must run inside the workspace root: {}",
            meta.working_copy.path.display()
        );
        // The session remembers its target — views group on this.
        assert_eq!(meta.workspace.as_deref(), Some("app"));

        // An unknown alias is a typed refusal.
        let err = conn
            .launch(None, None, Some("nope"), None, None, SIZE, in_place())
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::Rpc(ref e) if matches!(e, bao_protocol::WireError::UnknownWorkspace { .. })),
            "expected UnknownWorkspace, got {err:?}"
        );

        // A named profile resolves host-side to its argv.
        conn.registry_put(
            RegistryEntry::profile(
                "sleeper",
                vec!["bash".into(), "-c".into(), "sleep 30".into()],
            )
            .unwrap(),
        )
        .await
        .unwrap();
        let via_profile = conn
            .launch(None, None, None, Some("sleeper"), None, SIZE, in_place())
            .await
            .unwrap();
        assert!(
            via_profile.command.starts_with("bash"),
            "profile must supply the command: {}",
            via_profile.command
        );
        // Explicit command beats the profile — most explicit wins.
        let explicit = conn
            .launch(
                Some(Command::parse("bash -c 'sleep 30'").unwrap()),
                None,
                None,
                Some("sleeper"),
                None,
                SIZE,
                in_place(),
            )
            .await
            .unwrap();
        assert!(explicit.command.contains("sleep 30"));
        conn.stop(&via_profile.id).await.unwrap();
        conn.stop(&explicit.id).await.unwrap();

        // An unknown profile is a typed refusal.
        let err = conn
            .launch(None, None, None, Some("nope"), None, SIZE, in_place())
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::Rpc(ref e) if matches!(e, bao_protocol::WireError::UnknownProfile { .. })),
            "expected UnknownProfile, got {err:?}"
        );
    })
    .await;
    let _ = host.kill().await;
    first.expect("workspace phase 1 timed out");

    // Phase 2: a fresh daemon still resolves the alias — the registry is
    // host state, not client state.
    let mut host = common::start_host(&home, port).await;
    let second = tokio::time::timeout(Duration::from_secs(15), async {
        let mut conn = Conn::connect(&addr).await.unwrap();
        let list = conn.registry_list().await.unwrap();
        assert_eq!(list.len(), 2, "registry must survive a restart");
        let app = list.iter().find(|e| e.alias == "app").expect("app entry");
        assert!(app.root().unwrap().starts_with(&scratch));
        assert!(list.iter().any(|e| e.alias == "sleeper"));

        // Forget; sessions already launched against it are untouched.
        conn.registry_remove("app").await.unwrap();
        let list = conn.registry_list().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].alias, "sleeper");
    })
    .await;
    let _ = host.kill().await;
    second.expect("workspace phase 2 timed out");
}
