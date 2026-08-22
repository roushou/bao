//! Domain vocabulary: typed identities, commands, and the data structures
//! shared across the wire, the host, and the clients. Pure data and rules —
//! no I/O, no `tokio`, no process or filesystem access.

use std::{fmt, path::PathBuf, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::{error::Error, sandbox::Workspace};

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
/// unknown).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

/// The exact argv a session runs with. Parsed once (shell-words), then kept as
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
// Hostname
// ---------------------------------------------------------------------------

/// The identity of the machine a session lives on — a validated hostname.
/// The seed of location grouping: machines are grouped by this, so it must be
/// stable and never empty.
///
/// This is pure data: parsing and validation only. Resolving the *local*
/// machine's hostname (env + `hostname` command) is I/O and lives in
/// `bao-daemon`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Hostname(String);

impl Hostname {
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
    /// Display string of the command running this session (`args` joined),
    /// precomputed for cheap rendering. pi/Claude Code/Codex CLI are
    /// harnesses — the programs the session runs.
    pub command: String,
    /// Exact argv.
    pub args: Command,
    pub cwd: PathBuf,
    /// The workspace the session works in — where, its git identity, and the
    /// isolation claim.
    pub workspace: Workspace,
    pub created: u64,
    /// Machine the session lives on (the daemon's hostname). The seed of
    /// location grouping.
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
}
