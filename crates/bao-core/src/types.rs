//! Domain vocabulary: typed identities, addresses, commands, and data
//! structures shared across the wire, the host, and the clients.
//!
//! The point of this module is that nothing is passed around as a bare
//! string or untyped JSON anymore: addresses are `Addr`, session ids are
//! `SessionId`, session commands are `Command`, and session metadata is
//! the `SessionMeta` struct — one type from host to UI.

use std::{
    fmt,
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
    str::FromStr,
    sync::OnceLock,
};

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use serde::{Deserialize, Serialize};

use crate::{
    error::Error,
    sandbox::{SandboxKind, SandboxSpec, Workspace},
};

pub const DEFAULT_PORT: u16 = 14551;

/// Epoch milliseconds.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// A time source. [`Clock::system`] is the wall clock; tests inject a fake
/// to drive time-based derivation (idle alert) without waiting.
#[derive(Debug, Clone, Copy)]
pub struct Clock(pub fn() -> u64);

impl Clock {
    pub fn system() -> Self {
        Clock(now_ms)
    }

    pub fn now_ms(&self) -> u64 {
        (self.0)()
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::system()
    }
}

// ---------------------------------------------------------------------------
// Addr
// ---------------------------------------------------------------------------

/// Where the daemon listens or a client connects: a TCP host:port, or a
/// unix socket path for local-only transport. The transport is part of the
/// address, so dialing and binding are both driven by one value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Addr {
    /// TCP host:port (loopback by default; explicit `--port` for remote).
    Tcp { host: IpAddr, port: u16 },
    /// A unix socket path — local-only, trust via filesystem permissions.
    Unix(PathBuf),
}

impl Addr {
    pub fn local(port: u16) -> Self {
        Self::Tcp {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port,
        }
    }

    pub fn localhost() -> Self {
        Self::local(DEFAULT_PORT)
    }

    /// A unix-socket address for local-only transport.
    pub fn unix(path: impl Into<PathBuf>) -> Self {
        Self::Unix(path.into())
    }
}

impl Default for Addr {
    fn default() -> Self {
        Self::localhost()
    }
}

impl fmt::Display for Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Addr::Tcp { host, port } => write!(f, "{host}:{port}"),
            Addr::Unix(path) => write!(f, "unix:{}", path.display()),
        }
    }
}

impl FromStr for Addr {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Error> {
        if let Some(path) = s.strip_prefix("unix:") {
            if path.is_empty() {
                return Err(Error::BadAddr);
            }
            return Ok(Addr::Unix(PathBuf::from(path)));
        }
        let (host, port) = s.rsplit_once(':').ok_or(Error::BadAddr)?;
        let host = host.parse().map_err(|_| Error::BadAddr)?;
        let port = port.parse().map_err(|_| Error::BadAddr)?;
        Ok(Addr::Tcp { host, port })
    }
}

// ---------------------------------------------------------------------------
// SessionId
// ---------------------------------------------------------------------------

/// Identifies one unit of work (session). Short, generated, prefix-matchable.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(String);

impl SessionId {
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().simple().to_string()[..8].to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn matches_prefix(&self, prefix: &str) -> bool {
        self.0.starts_with(prefix)
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for SessionId {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

/// Lifecycle of a session, typed everywhere — never matched as a string.
/// The exit code rides with `Exited` so the type cannot express a running
/// session with a code, or an exited one without one (`None` = exited, code
/// unknown — legacy records, or a signal death we couldn't translate).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Registered, sandbox not yet built (the launch saga's first step).
    Preparing,
    /// Process spawned, awaiting its first output (harness is booting).
    Starting,
    Running,
    Exited(Option<i32>),
    /// The record and history survived, but the process is gone
    /// (daemon restart / reboot).
    Interrupted,
    /// The session's meta could not be read (corrupt file, or written by a
    /// newer Bao). Salvaged, never dropped — history stays viewable, and it
    /// can be removed. Needs a human.
    Damaged,
    /// Relocated to another machine (future; kept so the enum is closed).
    Moved,
}

impl<'de> Deserialize<'de> for Status {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        // Accept both the modern tagged form ({"exited": 1}) and the legacy
        // unit strings ("exited") old records wrote.
        let v = serde_json::Value::deserialize(d)?;
        match v {
            serde_json::Value::String(s) => match s.as_str() {
                "preparing" => Ok(Status::Preparing),
                "starting" => Ok(Status::Starting),
                "running" => Ok(Status::Running),
                // Legacy: the code was carried separately.
                "exited" => Ok(Status::Exited(None)),
                "interrupted" => Ok(Status::Interrupted),
                "damaged" => Ok(Status::Damaged),
                "moved" => Ok(Status::Moved),
                other => Err(D::Error::custom(format!("unknown status {other:?}"))),
            },
            serde_json::Value::Object(mut o) => {
                if let Some(code) = o.remove("exited") {
                    let code = serde_json::from_value(code).map_err(D::Error::custom)?;
                    Ok(Status::Exited(code))
                } else if o.contains_key("running") {
                    Ok(Status::Running)
                } else if o.contains_key("interrupted") {
                    Ok(Status::Interrupted)
                } else if o.contains_key("damaged") {
                    Ok(Status::Damaged)
                } else if o.contains_key("moved") {
                    Ok(Status::Moved)
                } else {
                    Err(D::Error::custom("missing status key"))
                }
            }
            other => Err(D::Error::custom(format!(
                "expected a status, got {other:?}"
            ))),
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Status::Preparing => "preparing",
            Status::Starting => "starting",
            Status::Running => "running",
            Status::Exited(_) => "exited",
            Status::Interrupted => "interrupted",
            Status::Damaged => "damaged",
            Status::Moved => "moved",
        };
        write!(f, "{s}")
    }
}

// ---------------------------------------------------------------------------
// Command
// ---------------------------------------------------------------------------

/// The exact argv an session runs with. Parsed once (shell-words), then kept as
/// the source of truth — display strings are not round-trip-safe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Command(Vec<String>);

impl Command {
    pub fn parse(s: &str) -> Result<Self, Error> {
        let args = split_words(s);
        if args.is_empty() {
            return Err(Error::EmptyCommand);
        }
        Ok(Self(args))
    }

    pub fn from_args(args: Vec<String>) -> Self {
        Self(args)
    }

    pub fn as_args(&self) -> &[String] {
        &self.0
    }

    pub fn first(&self) -> &str {
        self.0.first().map(String::as_str).unwrap_or("")
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn display(&self) -> String {
        self.0.join(" ")
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}

impl Default for Command {
    fn default() -> Self {
        Self::from_args(Vec::new())
    }
}

/// Split a command string into argv, honoring single/double quotes and
/// backslash escapes (a minimal shell-words parser).
fn split_words(s: &str) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    let (mut in_single, mut in_double) = (false, false);
    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '\\' if !in_single => match chars.peek() {
                Some(&n) => {
                    cur.push(n);
                    chars.next();
                }
                None => cur.push('\\'),
            },
            c if c.is_whitespace() && !in_single && !in_double => {
                if !cur.is_empty() {
                    args.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        args.push(cur);
    }
    args
}

// ---------------------------------------------------------------------------
// TerminalSize
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalSize {
    pub cols: u16,
    pub rows: u16,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self {
            cols: 120,
            rows: 40,
        }
    }
}

// ---------------------------------------------------------------------------
// WireBytes
// ---------------------------------------------------------------------------

/// A byte payload carried over the JSON wire as base64.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireBytes(pub Vec<u8>);

impl Serialize for WireBytes {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&B64.encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for WireBytes {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let encoded = String::deserialize(d)?;
        B64.decode(&encoded)
            .map(WireBytes)
            .map_err(serde::de::Error::custom)
    }
}

impl From<Vec<u8>> for WireBytes {
    fn from(v: Vec<u8>) -> Self {
        Self(v)
    }
}

impl From<&[u8]> for WireBytes {
    fn from(v: &[u8]) -> Self {
        Self(v.to_vec())
    }
}

impl std::ops::Deref for WireBytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// Hostname
// ---------------------------------------------------------------------------

/// The identity of the machine a session lives on — a validated hostname.
/// The seed of location grouping and the spatial the overview: machines are
/// grouped by this, so it must be stable and never empty.
///
/// [`Hostname::local`] resolves the local machine once per process (cached),
/// in this order (first non-empty wins): `BAO_HOST` env override → the
/// `hostname` command (authoritative on POSIX) → `HOSTNAME` env →
/// `"localhost"` (honest last resort — a machine with no name is still one
/// machine).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Hostname(String);

impl Hostname {
    /// The local machine's hostname, resolved once per process and cached.
    /// Deterministic: the same machine resolves to the same value across
    /// restarts (unless it is renamed).
    pub fn local() -> Self {
        static LOCAL: OnceLock<Hostname> = OnceLock::new();
        LOCAL
            .get_or_init(|| Hostname(Self::resolve_local()))
            .clone()
    }

    /// A hostname from a trusted source (config, tests). Rejects values
    /// that cannot identify a machine: empty, or containing whitespace or
    /// control characters.
    pub fn parse(s: &str) -> Result<Self, Error> {
        let trimmed = s.trim();
        if trimmed.is_empty() || trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(Error::BadHostname(trimmed.to_string()));
        }
        Ok(Hostname(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

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
}

impl fmt::Display for Hostname {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Hostname {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

// ---------------------------------------------------------------------------
// SessionMeta
// ---------------------------------------------------------------------------

/// The full, typed snapshot of a session — identity plus the *derived*
/// facts the daemon computes. One type from host to UI. Views are stateless:
/// they render this as-is and never derive anything from it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionMeta {
    pub id: SessionId,
    pub name: Option<String>,
    /// Display string of the harness command running this session (args
    /// joined). pi/Claude Code/Codex CLI are harnesses; the session is the
    /// running unit of work — this session. (`session` still parses, for old
    /// files.)
    #[serde(alias = "session")]
    pub harness: String,
    /// Exact argv.
    pub args: Command,
    pub cwd: PathBuf,
    /// The workspace the session works in — where, its git identity, and the
    /// isolation claim. (`env` still parses, for files/wires written before
    /// the rename.)
    #[serde(alias = "env")]
    pub workspace: Workspace,
    pub created: u64,
    /// Machine the session lives on (the daemon's hostname). The seed of
    /// location grouping and the spatial the overview.
    pub host: Hostname,
    pub status: Status,
    /// Epoch ms of the last session output (0 = none yet).
    pub last_activity: u64,
    /// ANSI-stripped, one-lined tail of the output.
    pub last_output: String,
    /// Derived by the daemon from facts (status/exit code/idle) — see
    /// [`crate::alert`]. Views never compute this.
    pub alert: Option<crate::alert::Alert>,
    /// Honest "is the session waiting for the human?": `None` = unknown.
    /// Only adapter status hooks (future) may set `Some`; never guessed.
    pub waiting_for_input: Option<bool>,
    /// Seconds since the session last produced output (0 = never). A daemon-
    /// derived time fact, stamped with the daemon's clock — views read it,
    /// they never compute it.
    pub idle_secs: u64,
    /// Seconds since the session was created. Same: daemon-stamped.
    pub age_secs: u64,
}

// ---------------------------------------------------------------------------
// SessionSpec
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// LaunchRequest
// ---------------------------------------------------------------------------

/// Wire payload for `Rpc::Launch`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchRequest {
    #[serde(default)]
    pub command: Option<Command>,
    #[serde(default)]
    pub dir: Option<PathBuf>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub size: TerminalSize,
    /// Requested isolation. `None` = resolve the strongest the machine
    /// offers for the launch dir.
    #[serde(default)]
    pub sandbox: SandboxSpec,
}

/// What the daemon says about itself — read by the client handshake and the
/// machine-facing views (sessions across machines, the RTS).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonInfo {
    /// The machine this daemon runs on.
    pub host: Hostname,
    /// Wire protocol version this daemon speaks.
    pub protocol_version: u32,
    /// Isolation backends this machine can provide. A client offers only
    /// these, never more.
    pub sandbox_backends: Vec<SandboxKind>,
}

// ---------------------------------------------------------------------------
// Text helpers
// ---------------------------------------------------------------------------

/// Terminal-text conveniences shared by the host (output snippets) and the
/// clients (the TUI/CLI rendering).
pub trait TermStrExt {
    /// Strip ANSI escape sequences (CSI and OSC, best-effort).
    fn strip_ansi(&self) -> String;
    /// Truncate to `max` characters, adding a trailing ellipsis if cut.
    fn truncate(&self, max: usize) -> String;
}

impl TermStrExt for str {
    fn strip_ansi(&self) -> String {
        let mut out = String::with_capacity(self.len());
        let mut chars = self.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\u{1b}' {
                match chars.peek() {
                    Some('[') => {
                        // CSI: consume until a final byte in @..~ (or end).
                        chars.next();
                        for n in chars.by_ref() {
                            if ('@'..='~').contains(&n) {
                                break;
                            }
                        }
                    }
                    Some(']') => {
                        // OSC: consume until BEL or ESC \.
                        chars.next();
                        loop {
                            match chars.next() {
                                Some('\u{07}') => break,
                                Some('\u{1b}') => {
                                    if chars.peek() == Some(&'\\') {
                                        chars.next();
                                    }
                                    break;
                                }
                                None => break,
                                Some(_) => {}
                            }
                        }
                    }
                    _ => {
                        // Lone ESC: drop it and the next char, best-effort.
                        chars.next();
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    fn truncate(&self, max: usize) -> String {
        let count = self.chars().count();
        let mut out: String = self.chars().take(max).collect();
        if count > max {
            out.push('…');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addr_tcp_parses_and_displays() {
        let a: Addr = "127.0.0.1:14551".parse().unwrap();
        assert_eq!(a, Addr::local(14551));
        assert_eq!(a.to_string(), "127.0.0.1:14551");
        assert!(matches!(
            Addr::localhost(),
            Addr::Tcp {
                port: DEFAULT_PORT,
                ..
            }
        ));
    }

    #[test]
    fn addr_unix_parses_and_displays() {
        let a: Addr = "unix:/tmp/bao.sock".parse().unwrap();
        assert_eq!(a, Addr::Unix("/tmp/bao.sock".into()));
        assert_eq!(a.to_string(), "unix:/tmp/bao.sock");
        assert!(matches!(Addr::unix("/s"), Addr::Unix(_)));
    }

    #[test]
    fn addr_rejects_garbage() {
        assert!("".parse::<Addr>().is_err());
        assert!("unix:".parse::<Addr>().is_err());
        assert!("nonsense".parse::<Addr>().is_err());
        assert!("host:notaport".parse::<Addr>().is_err());
    }

    #[test]
    fn hostname_parse_rejects_non_names() {
        assert!(Hostname::parse("").is_err());
        assert!(Hostname::parse("   ").is_err());
        assert!(Hostname::parse("my host").is_err());
        assert!(Hostname::parse("my\u{0}host").is_err());
    }

    #[test]
    fn hostname_parse_trims_and_accepts_valid() {
        let h = Hostname::parse("  my-host  ").unwrap();
        assert_eq!(h.as_str(), "my-host");
        assert_eq!(h.to_string(), "my-host");
        assert_eq!(Hostname::from_str("vps-01").unwrap().as_str(), "vps-01");
    }

    #[test]
    fn hostname_serde_round_trips_as_plain_string() {
        let h = Hostname::parse("vps-01").unwrap();
        let json = serde_json::to_string(&h).unwrap();
        assert_eq!(json, "\"vps-01\"");
        let back: Hostname = serde_json::from_str(&json).unwrap();
        assert_eq!(back, h);
    }

    #[test]
    fn hostname_local_never_empty() {
        assert!(!Hostname::local().as_str().is_empty());
    }
}
