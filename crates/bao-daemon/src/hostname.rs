//! Resolve the local machine's hostname — the one piece of I/O the daemon
//! performs to answer "what machine is this?". Pure validation stays in
//! `bao-core::types::Hostname`; the *resolution* (env + `hostname` command,
//! cached once per process) lives here.

use std::sync::OnceLock;

use bao_core::types::Hostname;

/// The local machine's hostname, resolved once per process and cached.
/// Deterministic: the same machine resolves to the same value across
/// restarts (unless it is renamed).
pub fn resolve() -> Hostname {
    static LOCAL: OnceLock<Hostname> = OnceLock::new();
    LOCAL
        .get_or_init(|| {
            let raw = resolve_local();
            Hostname::parse(&raw).unwrap_or_else(|_| Hostname::parse("localhost").unwrap())
        })
        .clone()
}

/// Resolve in this order (first non-empty wins): `BAO_HOST` env override →
/// the `hostname` command (authoritative on POSIX) → `HOSTNAME` env →
/// `"localhost"` (honest last resort — a machine with no name is still one
/// machine).
fn resolve_local() -> String {
    std::env::var("BAO_HOST")
        .ok()
        .or_else(|| {
            std::process::Command::new("hostname")
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        })
        .or_else(|| std::env::var("HOSTNAME").ok())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "localhost".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_is_never_empty() {
        assert!(!resolve().as_str().is_empty());
    }
}
