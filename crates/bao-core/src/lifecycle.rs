//! The session lifecycle state machine and the [`Transition`] convention
//! every state machine in Bao follows (see `docs/design/state-machines.md`).
//!
//! Pure: states and events in, a state out. No I/O, no `Session`, no side
//! effects — so the table is tested in isolation and shared by the daemon's
//! live write path and the restore fold.

use crate::{error::Error, types::Status};

/// The lifecycle's input alphabet — one variant per real fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleEvent {
    /// The process is up (`Preparing → Starting`; `Interrupted → Starting`
    /// on resume).
    Spawned,
    /// Session output (the first flips `Starting → Running`; the rest is steady).
    Output,
    /// Process exit, with its code (`Starting`/`Running → Exited`).
    Exited(Option<i32>),
    /// A daemon restart found a live state (`→ Interrupted`).
    Interrupted,
    /// Meta unreadable (`→ Damaged`).
    Damaged,
    /// Relocated to another machine (`→ Moved`; future).
    Moved,
}

impl LifecycleEvent {
    /// Short name, for the illegal-transition error.
    pub fn name(&self) -> &'static str {
        match self {
            LifecycleEvent::Spawned => "spawned",
            LifecycleEvent::Output => "output",
            LifecycleEvent::Exited(_) => "exited",
            LifecycleEvent::Interrupted => "interrupted",
            LifecycleEvent::Damaged => "damaged",
            LifecycleEvent::Moved => "moved",
        }
    }
}

/// A pure, context-free state machine: `(state, event) → next state`.
///
/// Guarded machines that need context or produce side-effecting actions are
/// *not* `Transition`s — they are Mealy machines with their own shape (see the
/// overview input machine).
pub trait Transition: Sized {
    type Event;

    /// The whole machine in one function. Every legal edge is a named pair;
    /// everything else is an error.
    fn apply(&self, event: &Self::Event) -> Result<Self, Error>;

    /// Fold a stream of events over a starting state. Lenient: an illegal
    /// event keeps the previous state, so restore never fails on a corrupt log.
    fn fold(self, events: impl IntoIterator<Item = Self::Event>) -> Self {
        events
            .into_iter()
            .fold(self, |s, e| s.apply(&e).unwrap_or(s))
    }
}

impl Transition for Status {
    type Event = LifecycleEvent;

    fn apply(&self, e: &LifecycleEvent) -> Result<Status, Error> {
        use LifecycleEvent as Ev;
        use Status as St;
        let next = match (*self, e) {
            // Launch step 2 / resume: a process is now up, awaiting first output.
            (St::Preparing, Ev::Spawned) | (St::Interrupted, Ev::Spawned) => St::Starting,
            // The first output flips a booting session to running; the rest is steady.
            (St::Starting, Ev::Output) | (St::Running, Ev::Output) => St::Running,
            // Process exit — legal from Starting (died before first output) too.
            (St::Starting, Ev::Exited(c)) | (St::Running, Ev::Exited(c)) => St::Exited(*c),
            // Restore honesty: a live state found on a fresh restore is interrupted.
            (St::Preparing, Ev::Interrupted)
            | (St::Starting, Ev::Interrupted)
            | (St::Running, Ev::Interrupted) => St::Interrupted,
            // Terminal states are reachable from anywhere (restore / migration).
            (_, Ev::Damaged) => St::Damaged,
            (_, Ev::Moved) => St::Moved,
            // Everything else is illegal.
            (from, event) => return Err(Error::IllegalTransition(from, event.name())),
        };
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_table_accepts_legal_cells() {
        use LifecycleEvent::*;
        // Launch: Preparing → Starting → Running.
        assert_eq!(Status::Preparing.apply(&Spawned).unwrap(), Status::Starting);
        assert_eq!(Status::Starting.apply(&Output).unwrap(), Status::Running);
        assert_eq!(
            Status::Running.apply(&Output).unwrap(),
            Status::Running,
            "steady state"
        );
        // Exit, including before first output.
        assert_eq!(
            Status::Starting.apply(&Exited(Some(1))).unwrap(),
            Status::Exited(Some(1))
        );
        // Resume and restore honesty.
        assert_eq!(
            Status::Interrupted.apply(&Spawned).unwrap(),
            Status::Starting
        );
        assert_eq!(
            Status::Running.apply(&Interrupted).unwrap(),
            Status::Interrupted
        );
        // Terminal states are reachable from anywhere.
        assert_eq!(Status::Running.apply(&Damaged).unwrap(), Status::Damaged);
    }

    #[test]
    fn transition_table_rejects_illegal_cells() {
        use LifecycleEvent::*;
        // Output without a live process.
        assert!(Status::Preparing.apply(&Output).is_err());
        assert!(Status::Interrupted.apply(&Output).is_err());
        assert!(Status::Exited(Some(1)).apply(&Output).is_err());
        // Becoming live when already live, or from a terminal state.
        assert!(Status::Running.apply(&Spawned).is_err());
        assert!(Status::Starting.apply(&Spawned).is_err());
        assert!(Status::Exited(Some(1)).apply(&Spawned).is_err());
        assert!(Status::Damaged.apply(&Spawned).is_err());
    }

    #[test]
    fn fold_is_lenient_on_illegal_events() {
        use LifecycleEvent::*;
        // Output before spawn is illegal; the fold keeps the previous state.
        let state = Status::Preparing.fold([Output, Spawned, Output]);
        assert_eq!(state, Status::Running);
    }
}
