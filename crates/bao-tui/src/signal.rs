//! The semantic status language — the one table for every view.
//!
//! glyph + color + words, triple-encoded, so the meaning survives
//! color-blindness, monochrome terminals, and copy-paste. Used identically
//! by the rail rows, the palette, and the terminal title — a view can never
//! drift from another.
//!
//! Everything here derives from daemon facts (`Status`, `Alert`,
//! `waiting_for_input`, `idle_secs`) — nothing is ever guessed.

use bao_core::{alert::Alert, types::Status};
use ratatui::style::Style;

use crate::theme;

/// The display of one session's signal, computed by the one precedence rule:
/// data at risk < errored < interrupted < proven waiting < idle.
#[derive(Debug, Clone)]
pub struct Signal {
    /// The status glyph.
    pub glyph: char,
    /// The semantic color.
    pub style: Style,
    /// The short status sentence (e.g. "idle 3m").
    pub text: String,
    /// The "why", in words (e.g. "quiet for 3m — no output since"). Tested,
    /// but not yet surfaced in a view.
    #[allow(dead_code)]
    pub rationale: String,
    /// Severity within the NEEDS YOU group (meaningless elsewhere).
    pub rank: u8,
    /// Whether the proven-waiting signal is the one to show.
    pub waiting_visible: bool,
}

/// Is the proven-waiting signal the one to show? Waiting is displayed only
/// when a more severe fact (data at risk, failed, interrupted) does not
/// overshadow it. In real data these are mutually exclusive; the ordering is
/// defensive so no view can ever contradict itself.
fn waiting_visible(alert: Option<Alert>, waiting: bool) -> bool {
    waiting
        && !matches!(
            alert,
            Some(Alert::Damaged | Alert::Errored(_) | Alert::Interrupted)
        )
}

/// The one status-language table, computed from daemon facts.
pub fn signal(status: Status, alert: Option<Alert>, waiting: bool, idle_secs: u64) -> Signal {
    let waiting = waiting_visible(alert, waiting);
    let (glyph, style, text, rationale, rank) = if waiting {
        (
            '◉',
            Style::default().fg(theme::palette().waiting),
            if idle_secs > 0 {
                format!("waiting for you · idle {idle_secs}s")
            } else {
                "waiting for you".to_string()
            },
            "waiting for you — the harness reported it".to_string(),
            3,
        )
    } else {
        match alert {
            Some(Alert::Damaged) => (
                '⚠',
                Style::default().fg(theme::palette().damaged),
                "damaged — needs alert".to_string(),
                "damaged — meta unreadable, needs a human".to_string(),
                0,
            ),
            Some(Alert::Errored(c)) => (
                '✕',
                Style::default().fg(theme::palette().errored),
                format!("errored (code {c})"),
                format!("errored — exited with code {c}"),
                1,
            ),
            Some(Alert::Interrupted) => (
                '⏸',
                Style::default().fg(theme::palette().interrupted),
                "interrupted — needs action".to_string(),
                "interrupted — process gone, needs resume or remove".to_string(),
                2,
            ),
            Some(Alert::Idle(s)) => (
                '…',
                Style::default().fg(theme::palette().interrupted),
                format!("idle {s}s"),
                format!("quiet for {} — no output since", fmt_age(s)),
                4,
            ),
            Some(Alert::Done) => (
                '✓',
                Style::default().fg(theme::palette().done),
                "done".to_string(),
                "done — finished cleanly".to_string(),
                0,
            ),
            None => match status {
                Status::Running => (
                    '●',
                    Style::default(),
                    if idle_secs == 0 {
                        "running · no output yet".to_string()
                    } else {
                        format!("running · idle {idle_secs}s")
                    },
                    if idle_secs == 0 {
                        "running — no output yet".to_string()
                    } else {
                        format!("running · idle {idle_secs}s")
                    },
                    0,
                ),
                Status::Preparing => (
                    theme::glyphs().hollow,
                    Style::default().fg(theme::palette().dim),
                    "preparing".to_string(),
                    "launching — building the sandbox".to_string(),
                    0,
                ),
                Status::Starting => (
                    theme::glyphs().hollow,
                    Style::default().fg(theme::palette().accent),
                    "starting".to_string(),
                    "launched — waiting for first output".to_string(),
                    0,
                ),
                other => (
                    '·',
                    Style::default().fg(theme::palette().done),
                    other.to_string(),
                    other.to_string(),
                    0,
                ),
            },
        }
    };
    Signal {
        glyph,
        style,
        text,
        rationale,
        rank,
        waiting_visible: waiting,
    }
}

/// Seconds → "42s" / "12m" / "3h".
pub(crate) fn fmt_age(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}

#[cfg(test)]
mod tests {
    use bao_core::alert::AlertInput;

    use super::*;

    fn sig(status: Status, idle_secs: u64, waiting: bool) -> Signal {
        let alert = AlertInput { status, idle_secs }.alert();
        signal(status, alert, waiting, idle_secs)
    }

    #[test]
    fn glyphs_follow_the_semantic_table() {
        assert_eq!(sig(Status::Running, 5, false).glyph, '●');
        assert_eq!(
            sig(Status::Running, 0, false).text,
            "running · no output yet"
        );
        assert_eq!(sig(Status::Running, 200, false).glyph, '…');
        assert_eq!(
            sig(Status::Running, 200, true).glyph,
            '◉',
            "waiting outranks idle"
        );
        assert_eq!(sig(Status::Exited(Some(1)), 0, false).glyph, '✕');
        assert_eq!(sig(Status::Exited(Some(0)), 0, false).glyph, '✓');
        assert_eq!(sig(Status::Interrupted, 0, false).glyph, '⏸');
        assert_eq!(sig(Status::Damaged, 0, false).glyph, '⚠');
    }

    #[test]
    fn preparing_and_starting_are_boot_states() {
        let hollow = crate::theme::glyphs().hollow;
        assert_eq!(sig(Status::Preparing, 0, false).glyph, hollow);
        assert_eq!(sig(Status::Starting, 0, false).glyph, hollow);
        assert_eq!(sig(Status::Preparing, 0, false).text, "preparing");
        assert_eq!(sig(Status::Starting, 0, false).text, "starting");
        assert!(
            sig(Status::Starting, 0, false)
                .rationale
                .contains("first output")
        );
        assert!(
            sig(Status::Preparing, 0, false)
                .rationale
                .contains("sandbox")
        );
    }

    #[test]
    fn precedence_is_defensive() {
        // A contradictory waiting + damaged still shows damaged (data at
        // risk outranks everything) — the view can never contradict itself.
        let s = signal(Status::Running, Some(Alert::Damaged), true, 0);
        assert_eq!(s.glyph, '⚠');
        assert!(!s.waiting_visible);
    }

    #[test]
    fn rationale_states_the_why() {
        assert!(
            sig(Status::Running, 120, false)
                .rationale
                .contains("quiet for 2m")
        );
        assert_eq!(
            sig(Status::Exited(Some(1)), 0, false).rationale,
            "errored — exited with code 1"
        );
        assert!(
            sig(Status::Running, 200, true)
                .rationale
                .contains("the harness reported it")
        );
    }

    #[test]
    fn rank_orders_need_within_the_group() {
        assert_eq!(sig(Status::Damaged, 0, false).rank, 0);
        assert_eq!(sig(Status::Exited(Some(1)), 0, false).rank, 1);
        assert_eq!(sig(Status::Interrupted, 0, false).rank, 2);
        assert_eq!(sig(Status::Running, 200, true).rank, 3, "waiting < idle");
        assert_eq!(sig(Status::Running, 200, false).rank, 4);
    }

    #[test]
    fn age_formats_seconds_minutes_hours() {
        assert_eq!(fmt_age(42), "42s");
        assert_eq!(fmt_age(720), "12m");
        assert_eq!(fmt_age(10_800), "3h");
    }
}
