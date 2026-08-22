//! Multi-session slice: two sessions in the same repo, each in its own isolated
//! worktree; named sessions; attach by name; rm removes the worktree.

mod common;

use std::{
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
    str::FromStr,
};

use bao_client::Conn;
use bao_core::types::{Command, TerminalSize};

fn git(cwd: &Path, args: &[&str]) {
    let st = ProcessCommand::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .status()
        .unwrap();
    assert!(st.success(), "git {args:?} failed in {}", cwd.display());
}

fn make_repo() -> PathBuf {
    let repo = std::env::temp_dir().join(format!("bao-multi-repo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&repo);
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "test@bao"]);
    git(&repo, &["config", "user.name", "bao test"]);
    std::fs::write(repo.join("main.txt"), "hello\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "init"]);
    repo
}

fn launcher(echo: &str) -> String {
    format!("bash -c 'echo {echo}; while read -r line; do echo \"got:$line\"; done'")
}

#[tokio::test]
async fn two_agents_share_a_repo_without_stepping_on_each_other() {
    let home = common::unique_home("multi");
    let port = common::free_port();
    let addr = common::addr(port);
    let repo = make_repo();
    let mut host = common::start_host(&home, port).await;

    // Launch session A (writes a marker file) and session B in the same repo.
    let mut a = Conn::connect(&addr).await.unwrap();
    let meta_a = a
        .launch(
            Some(Command::parse(&launcher("A > marker.txt; echo READY_A")).unwrap()),
            Some(repo.clone()),
            Some("alpha".to_string()),
            TerminalSize {
                cols: 120,
                rows: 40,
            },
            bao_core::sandbox::SandboxSpec::default(),
        )
        .await
        .unwrap();
    let sid_a = meta_a.id.clone();
    common::wait_for_output(&mut a, "READY_A", "session A starts").await;

    let mut b = Conn::connect(&addr).await.unwrap();
    let meta_b = b
        .launch(
            Some(Command::parse(&launcher("echo READY_B")).unwrap()),
            Some(repo.clone()),
            Some("beta".to_string()),
            TerminalSize {
                cols: 120,
                rows: 40,
            },
            bao_core::sandbox::SandboxSpec::default(),
        )
        .await
        .unwrap();
    let sid_b = meta_b.id.clone();
    common::wait_for_output(&mut b, "READY_B", "session B starts").await;
    assert_ne!(sid_a, sid_b, "two launches must be two sessions");

    // The launch reply is the `Preparing` snapshot (provisional sandbox);
    // the background saga materializes the real sandbox. Re-read the live
    // metas now that both sessions are running.
    let sessions = a.list().await.unwrap();
    let meta_a = sessions.iter().find(|s| s.id == sid_a).unwrap().clone();
    let meta_b = sessions.iter().find(|s| s.id == sid_b).unwrap().clone();

    // Both workspaces must be distinct worktrees, not the repo itself.
    let env_a = &meta_a.workspace;
    let env_b = &meta_b.workspace;
    assert_eq!(env_a.kind, bao_core::sandbox::SandboxKind::Worktree);
    assert_eq!(env_b.kind, bao_core::sandbox::SandboxKind::Worktree);
    let path_a = env_a.path.clone();
    let path_b = env_b.path.clone();
    assert_ne!(path_a, path_b, "sessions must get different worktrees");
    assert_ne!(path_a, repo);
    assert_ne!(path_b, repo);
    assert_ne!(env_a.branch, env_b.branch);

    // Isolation: A's marker file must not leak into B's worktree or the repo.
    assert!(path_a.join("marker.txt").exists(), "A wrote its marker");
    assert!(
        !path_b.join("marker.txt").exists(),
        "B's worktree must not see A's file"
    );
    assert!(
        !repo.join("marker.txt").exists(),
        "the main checkout must stay clean"
    );

    // list shows both, by name, with branches.
    let mut l = Conn::connect(&addr).await.unwrap();
    let sessions = l.list().await.unwrap();
    assert_eq!(sessions.len(), 2);
    let names: Vec<&str> = sessions.iter().filter_map(|s| s.name.as_deref()).collect();
    assert!(names.contains(&"alpha") && names.contains(&"beta"));

    // Attach by name replays the right session.
    let mut c = Conn::connect(&addr).await.unwrap();
    let (attached, _seq, screen) = c
        .attach(&bao_core::types::SessionId::from_str("alpha").unwrap())
        .await
        .unwrap();
    assert_eq!(attached.id, sid_a);
    let text = String::from_utf8_lossy(&screen);
    assert!(
        text.contains("READY_A"),
        "attached to the wrong session (screen: {text})"
    );

    // rm removes the session AND its worktree; the repo is untouched.
    let mut d = Conn::connect(&addr).await.unwrap();
    d.rm(&bao_core::types::SessionId::from_str("alpha").unwrap())
        .await
        .unwrap();
    assert!(!path_a.exists(), "worktree must be removed");
    let branch_after = ProcessCommand::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["branch", "--list", env_a.branch.as_deref().unwrap()])
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&branch_after.stdout)
            .trim()
            .is_empty(),
        "session branch must be deleted"
    );
    assert!(repo.join("main.txt").exists(), "repo untouched");
    assert!(
        !repo.join("marker.txt").exists(),
        "repo must not have inherited A's file"
    );

    host.kill().await.unwrap();
    let _ = std::fs::remove_dir_all(&home);
    let _ = std::fs::remove_dir_all(&repo);
}
