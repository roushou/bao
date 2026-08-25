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
    use bao_core::sandbox::WorkingCopy;

    use super::*;

    #[test]
    fn fallback_is_honest_about_capabilities() {
        let f = Fallback;
        assert_eq!(f.resume_args(&WorkingCopy::default()), None);
        assert_eq!(f.waiting_for_input(&WorkingCopy::default()), None);
        assert!(f.pack(&WorkingCopy::default()).is_err());
        assert!(f.unpack(&WorkingCopy::default(), &[]).is_err());
    }
}
