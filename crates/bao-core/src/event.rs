//! The session event vocabulary: the log's entries and the lifecycle fold.

use std::collections::VecDeque;

use crate::{
    lifecycle::{LifecycleEvent, Transition},
    types::{SessionId, Status},
};

#[derive(Debug, Clone)]
pub enum EventKind {
    Output(Vec<u8>),
    Status(Status),
}

#[derive(Debug, Clone)]
pub struct SessionEvent {
    pub session: SessionId,
    pub seq: u64,
    pub ts: u64,
    pub kind: EventKind,
}

/// Map a persisted log entry onto the lifecycle's input alphabet. `None` for
/// entries that carry no lifecycle fact — a `Preparing` status never appears in
/// the log (it is the fold's seed).
fn lifecycle_event(kind: &EventKind) -> Option<LifecycleEvent> {
    match kind {
        EventKind::Output(_) => Some(LifecycleEvent::Output),
        EventKind::Status(st) => match st {
            Status::Starting => Some(LifecycleEvent::Spawned),
            // The logged `running` transition records the fact the first
            // output produced — fold it as output (idempotent from either
            // Starting or Running).
            Status::Running => Some(LifecycleEvent::Output),
            Status::Exited(c) => Some(LifecycleEvent::Exited(*c)),
            Status::Interrupted => Some(LifecycleEvent::Interrupted),
            Status::Damaged => Some(LifecycleEvent::Damaged),
            Status::Moved => Some(LifecycleEvent::Moved),
            Status::Preparing => None,
        },
    }
}

/// Derive a restored session's lifecycle state: fold the event log from the
/// `Preparing` seed. The honesty rule applies after: any live state found
/// on a fresh restore is `Interrupted` — the process is gone.
pub fn fold_status(log: &VecDeque<(u64, EventKind)>) -> Status {
    let last = Status::Preparing.fold(log.iter().filter_map(|(_, k)| lifecycle_event(k)));
    match last {
        // The process is gone; nothing that looked live can still be running.
        Status::Preparing | Status::Starting | Status::Running => Status::Interrupted,
        other => other,
    }
}
