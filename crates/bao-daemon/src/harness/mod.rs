//! The known harnesses and the contract that drives them.
//!
//! The `Harness` trait (the adapter contract) lives here with the
//! implementations, one module per harness, plus the registry that
//! enumerates them. A new harness is one new file here and one line in
//! `HarnessRegistry::KNOWN` (fallback last) — the contract test
//! `every_known_harness_is_reachable` enforces the wiring.

use bao_core::{sandbox::WorkingCopy, types::Command};

use error::Error;

pub mod error;
mod fallback;
mod pi;

pub use fallback::Fallback;
pub use pi::Pi;

/// The adapter contract for a kind of coding-session harness: everything Bao
/// knows about driving *that* harness lives in its impl — never in free
/// functions. Implementations are stateless unit structs; the registry owns
/// identification.
pub trait Harness: Send + Sync {
    /// Harness name.
    fn name(&self) -> &'static str;

    /// Does this command invoke this harness? (registry identification)
    fn matches(&self, command: &Command) -> bool;

    /// Extra argv to relaunch with its conversation intact. `None` = honest
    /// fresh start (the harness persists whatever it persists).
    fn resume_args(&self, _sandbox: &WorkingCopy) -> Option<Vec<String>> {
        None
    }

    /// Honest "is it waiting for the human?" `None` = we cannot tell.
    /// Future: feeds `SessionMeta.waiting_for_input`. Never guessed.
    fn waiting_for_input(&self, _sandbox: &WorkingCopy) -> Option<bool> {
        None
    }

    /// Serialize conversation state for relocation. Future: the move slice.
    /// Default = cannot pack (honest unsupported).
    fn pack(&self, _sandbox: &WorkingCopy) -> Result<Vec<u8>, Error> {
        Err(Error::Unsupported("pack"))
    }

    /// Restore conversation state after relocation. Future: the move slice.
    fn unpack(&self, _sandbox: &WorkingCopy, _data: &[u8]) -> Result<(), Error> {
        Err(Error::Unsupported("unpack"))
    }
}

static PI: Pi = Pi;
static FALLBACK: Fallback = Fallback;

/// The enumerated set of known harnesses. Totality lives here: one entry per
/// harness, fallback last. A later plugin story swaps this static list for a
/// runtime registry without touching the trait or its callers.
pub struct HarnessRegistry;

impl HarnessRegistry {
    const KNOWN: &'static [&'static dyn Harness] = &[&PI, &FALLBACK];

    /// Identify the harness behind a command. First match wins; the fallback
    /// matches anything and must be last.
    pub fn identify(command: &Command) -> &'static dyn Harness {
        Self::KNOWN
            .iter()
            .find(|h| h.matches(command))
            .copied()
            .unwrap_or(&FALLBACK)
    }
}

#[cfg(test)]
mod tests {
    use bao_core::types::Command;

    use super::*;

    fn cmd(s: &str) -> Command {
        Command::parse(s).unwrap()
    }

    #[test]
    fn identify_resolves_known_harnesses() {
        assert_eq!(
            HarnessRegistry::identify(&cmd("pi --config x")).name(),
            "pi"
        );
    }

    #[test]
    fn identify_falls_back_for_unknown_commands() {
        assert_eq!(
            HarnessRegistry::identify(&cmd("claude -p hi")).name(),
            "fallback"
        );
        assert_eq!(
            HarnessRegistry::identify(&cmd("codex exec")).name(),
            "fallback"
        );
    }

    #[test]
    fn every_known_harness_is_reachable() {
        let names: Vec<&str> = HarnessRegistry::KNOWN.iter().map(|h| h.name()).collect();
        assert!(names.contains(&"pi"), "pi must be registered");
        assert!(names.contains(&"fallback"), "fallback must be registered");
    }
}
