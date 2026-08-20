//! Alert signals — derived *by the daemon*, never by views.
//!
//! Honesty invariant: every signal derives from a known fact (status, exit
//! code, last activity). Nothing here guesses what an session is doing; if we
//! can't tell, we say nothing.

use serde::{Deserialize, Serialize};

use crate::types::Status;

/// How much idle time (seconds) before a running session is flagged for
/// alert. A fact (idle since last activity), never a guess about intent.
pub const IDLE_ALERT_SECS: u64 = 60;

/// Why a session needs (or has finished needing) the human's alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Alert {
    /// Running but no output for `secs` — a fact, not a guess.
    Idle(u64),
    /// Exited with a non-zero code.
    Errored(i32),
    /// Process gone (daemon restart/reboot) — needs resume or rm.
    Interrupted,
    /// The session's meta could not be read — data at risk, needs a human.
    Damaged,
    /// Finished cleanly.
    Done,
}

impl Alert {
    /// Sort key: lower = needs the human sooner.
    pub fn rank(&self) -> u8 {
        match self {
            Alert::Damaged => 0,
            Alert::Errored(_) => 1,
            Alert::Interrupted => 2,
            Alert::Idle(_) => 3,
            Alert::Done => 5,
        }
    }
}

/// Everything the alert judgment needs to know about a session. The
/// time fact (`idle_secs`) is *provided* by the daemon's clock, not computed
/// here — the judgment is a method on its inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlertInput {
    pub status: Status,
    /// How long the session has been quiet — stamped by the daemon.
    pub idle_secs: u64,
}

impl AlertInput {
    /// Does this state need a human, and why? Derived only from the inputs
    /// above — nothing here guesses what an session is doing.
    pub fn alert(&self) -> Option<Alert> {
        match self.status {
            Status::Exited(code) => match code {
                Some(c) if c != 0 => Some(Alert::Errored(c)),
                _ => Some(Alert::Done),
            },
            Status::Interrupted => Some(Alert::Interrupted),
            Status::Damaged => Some(Alert::Damaged),
            Status::Running => {
                if self.idle_secs >= IDLE_ALERT_SECS {
                    Some(Alert::Idle(self.idle_secs))
                } else {
                    None
                }
            }
            // Booting, not yet producing output: never alert — just
            // not there yet. Honest: we cannot tell what it's doing.
            Status::Preparing | Status::Starting => None,
            Status::Moved => None,
        }
    }
}

/// Seconds since the session last produced output (0 = never produced any).
/// The daemon's time fact — computed once, stamped onto [`SessionMeta`] so
/// views never derive it with their own clock.
pub fn idle_secs(last_activity: u64, now: u64) -> u64 {
    if last_activity == 0 {
        0
    } else {
        now.saturating_sub(last_activity) / 1000
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(status: Status, idle_secs: u64) -> AlertInput {
        AlertInput { status, idle_secs }
    }

    #[test]
    fn running_and_active_is_quiet() {
        assert_eq!(input(Status::Running, 5).alert(), None);
    }

    #[test]
    fn running_and_idle_gets_alert() {
        // Exactly at the threshold.
        assert_eq!(
            input(Status::Running, IDLE_ALERT_SECS).alert(),
            Some(Alert::Idle(IDLE_ALERT_SECS))
        );
        // Well past it.
        assert_eq!(input(Status::Running, 120).alert(), Some(Alert::Idle(120)));
    }

    #[test]
    fn fresh_silent_agent_is_not_idle() {
        // No output yet (idle 0s) must never be flagged idle.
        assert_eq!(input(Status::Running, 0).alert(), None);
    }

    #[test]
    fn errored_exit_needs_alert() {
        assert_eq!(
            input(Status::Exited(Some(1)), 0).alert(),
            Some(Alert::Errored(1))
        );
    }

    #[test]
    fn clean_exit_is_done() {
        assert_eq!(input(Status::Exited(Some(0)), 0).alert(), Some(Alert::Done));
        assert_eq!(input(Status::Exited(None), 0).alert(), Some(Alert::Done));
    }

    #[test]
    fn interrupted_needs_action() {
        assert_eq!(
            input(Status::Interrupted, 0).alert(),
            Some(Alert::Interrupted)
        );
    }

    #[test]
    fn damaged_is_flagged_above_all() {
        assert_eq!(input(Status::Damaged, 0).alert(), Some(Alert::Damaged));
    }

    #[test]
    fn moved_is_quiet() {
        assert_eq!(input(Status::Moved, 0).alert(), None);
    }

    #[test]
    fn idle_secs_uses_whole_seconds() {
        assert_eq!(idle_secs(1_000_000, 1_100_000), 100);
        assert_eq!(idle_secs(1_000_000, 1_000_500), 0);
        assert_eq!(idle_secs(0, 1_000_000), 0, "never output -> 0");
    }

    #[test]
    fn rank_orders_alert() {
        assert!(Alert::Damaged.rank() < Alert::Errored(1).rank());
        assert!(Alert::Errored(1).rank() < Alert::Interrupted.rank());
        assert!(Alert::Interrupted.rank() < Alert::Idle(1).rank());
        assert!(Alert::Idle(1).rank() < Alert::Done.rank());
    }
}
