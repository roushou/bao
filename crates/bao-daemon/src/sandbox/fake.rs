//! Test doubles: a fake backend and factory that do no I/O, so the registry
//! and saga are testable without git, disk, or sandbox binaries.

use std::path::Path;

use bao_core::{
    sandbox::{SandboxKind, WorkingCopy},
    types::SessionId,
};

use crate::error::Error;

use super::{Sandbox, SandboxBackend, SandboxFactory, SandboxSpec, WorkingCopyStore};

/// A backend that materializes no working copy: `prepare` points at the cwd,
/// and `teardown` is a no-op. `wrap_command` stays the trait default (launch
/// unchanged).
#[derive(Debug)]
pub(crate) struct FakeBackend {
    pub(crate) kind: SandboxKind,
}

impl SandboxBackend for FakeBackend {
    fn kind(&self) -> SandboxKind {
        self.kind
    }

    fn prepare(&self, _id: &SessionId, cwd: &Path) -> Result<WorkingCopy, Error> {
        Ok(WorkingCopy {
            kind: self.kind,
            repo: None,
            branch: None,
            path: cwd.to_path_buf(),
        })
    }

    fn teardown(&self, _workspace: &WorkingCopy) -> Result<(), Error> {
        Ok(())
    }
}

/// A factory that materializes a [`FakeBackend`]. Set `fail` to make
/// `materialize` error — the deterministic way to exercise saga compensation.
#[derive(Debug, Clone)]
pub(crate) struct FakeSandboxFactory {
    pub(crate) kind: SandboxKind,
    pub(crate) fail: bool,
}

impl SandboxFactory for FakeSandboxFactory {
    fn materialize(
        &self,
        _store: &WorkingCopyStore,
        id: &SessionId,
        cwd: &Path,
        _spec: &SandboxSpec,
    ) -> Result<Sandbox, Error> {
        if self.fail {
            return Err(Error::SandboxUnavailable(self.kind));
        }
        let backend = FakeBackend { kind: self.kind };
        let working_copy = backend.prepare(id, cwd)?;
        Ok(Sandbox::new(working_copy, Box::new(backend)))
    }
}
