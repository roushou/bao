//! Shared helpers for integration tests in this package.
#![allow(dead_code)]

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use bao_client::{Conn, HostEvent};
use bao_transport::Addr;

/// A fresh non-git directory (launch dir for tests, so the daemon doesn't create
/// worktrees in the dev repo).
pub fn scratch_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bao-scratch-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn unique_home(label: &str) -> PathBuf {
    let home = std::env::temp_dir().join(format!("bao-test-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    home
}

pub fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

/// The explicit in-place spec for tests launching in a scratch (non-git)
/// directory — the default spec is a worktree, which requires a repo.
pub fn in_place() -> bao_core::sandbox::SandboxSpec {
    bao_core::sandbox::SandboxSpec {
        isolation: bao_core::sandbox::SandboxKind::InPlace,
    }
}

pub async fn start_host(home: &Path, port: u16) -> tokio::process::Child {
    let child = tokio::process::Command::new(env!("CARGO_BIN_EXE_bao"))
        .arg("daemon")
        .arg("--port")
        .arg(port.to_string())
        .env("BAO_HOME", home)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    for _ in 0..100 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            return child;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("bao daemon did not start listening");
}

pub fn addr(port: u16) -> Addr {
    Addr::local(port)
}

/// Drain events until decoded output contains `needle`, or fail.
pub async fn wait_for_output(conn: &mut Conn, needle: &str, label: &str) -> String {
    let mut acc = String::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        if acc.contains(needle) {
            return acc;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timeout waiting for {label} ({needle:?}); saw: {acc:?}"
        );
        match conn.next_event().await {
            Some(HostEvent::Output { data, .. }) => {
                acc.push_str(&String::from_utf8_lossy(&data));
            }
            Some(HostEvent::Disconnected) | None => panic!("{label}: host disconnected"),
            Some(_) => {}
        }
    }
}
