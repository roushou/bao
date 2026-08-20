//! The honest catch-all harness: any command we don't recognize. It
//! relaunches fresh (the harness persists whatever it persists) and claims
//! no capability it doesn't have — every default is an honest absence.

use bao_core::types::Command;

use super::Harness;

/// The honest catch-all: any command we don't recognize.
#[derive(Debug, Clone, Copy, Default)]
pub struct Fallback;

impl Harness for Fallback {
    fn name(&self) -> &'static str {
        "fallback"
    }

    fn matches(&self, _command: &Command) -> bool {
        true // the registry orders it last
    }
}

#[cfg(test)]
mod tests {
    use bao_core::sandbox::Workspace;

    use super::*;

    #[test]
    fn fallback_is_honest_about_capabilities() {
        let f = Fallback;
        assert_eq!(f.resume_args(&Workspace::default()), None);
        assert_eq!(f.waiting_for_input(&Workspace::default()), None);
        assert!(f.pack(&Workspace::default()).is_err());
        assert!(f.unpack(&Workspace::default(), &[]).is_err());
    }
}
