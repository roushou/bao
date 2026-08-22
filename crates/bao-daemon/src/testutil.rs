//! Test-only helpers shared across the daemon's modules.

use std::path::{Path, PathBuf};

use bao_core::{
    sandbox::{SandboxKind, Workspace},
    types::SessionId,
};

use crate::{error::Error, sandbox::SandboxBackend};

/// A sandbox backend that materializes a throwaway directory under a fixed
/// temp root, so a test can observe `prepare`/`teardown` through the
/// filesystem without touching git or bubblewrap.
#[derive(Debug)]
pub(crate) struct FakeSandboxBackend;

impl FakeSandboxBackend {
    pub(crate) fn root() -> PathBuf {
        std::env::temp_dir().join(format!("bao-fake-sandbox-{}", std::process::id()))
    }
}

impl SandboxBackend for FakeSandboxBackend {
    fn kind(&self) -> SandboxKind {
        SandboxKind::InPlace
    }

    fn prepare(&self, id: &SessionId, _cwd: &Path) -> Result<Workspace, Error> {
        let path = Self::root().join(id.as_str());
        std::fs::create_dir_all(&path)?;
        Ok(Workspace {
            kind: SandboxKind::InPlace,
            repo: None,
            branch: None,
            path,
        })
    }

    fn teardown(&self, workspace: &Workspace) -> Result<(), Error> {
        let _ = std::fs::remove_dir_all(&workspace.path);
        Ok(())
    }
}
