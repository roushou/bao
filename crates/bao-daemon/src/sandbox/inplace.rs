//! The in-place backend: the user's own directory, no isolation.

use std::path::Path;

use bao_core::{
    sandbox::{SandboxKind, WorkingCopy},
    types::SessionId,
};

use crate::error::Error;

use super::SandboxBackend;

/// Runs the session in the user's own directory — no isolation.
#[derive(Debug)]
pub struct InPlace;

impl SandboxBackend for InPlace {
    fn kind(&self) -> SandboxKind {
        SandboxKind::InPlace
    }

    fn prepare(&self, _id: &SessionId, cwd: &Path) -> Result<WorkingCopy, Error> {
        Ok(WorkingCopy {
            kind: SandboxKind::InPlace,
            repo: None,
            branch: None,
            path: cwd.to_path_buf(),
        })
    }

    fn teardown(&self, _workspace: &WorkingCopy) -> Result<(), Error> {
        // The user's own directory is never touched.
        Ok(())
    }
}
