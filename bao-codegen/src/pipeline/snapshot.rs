//! Pipeline snapshot plugin for visualization and debugging.
//!
//! This module provides a plugin that captures the pipeline state after each phase,
//! enabling visualization of the compilation process.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::RwLock,
};

use eyre::Result;
use serde::Serialize;

use super::{CompilationContext, Diagnostic, Plugin};
use crate::schema::ComputedData;

/// A snapshot of the pipeline state at a specific phase.
#[derive(Debug, Clone, Serialize)]
pub struct PhaseSnapshot {
    /// The phase that just completed.
    pub phase: String,

    /// The Application IR (available after "lower" phase).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ir: Option<baobao_ir::AppIR>,

    /// Pre-computed analysis data (available after "analyze" phase).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computed: Option<ComputedData>,

    /// Diagnostics collected so far.
    pub diagnostics: Vec<Diagnostic>,
}

/// A plugin that captures pipeline state after each phase.
///
/// Use this plugin with the `--visualize` flag to output intermediate
/// representations for debugging and understanding the pipeline.
///
/// # Example
///
/// ```ignore
/// let snapshot_plugin = SnapshotPlugin::new();
/// let pipeline = Pipeline::new().plugin(snapshot_plugin.clone());
/// let ctx = pipeline.run(manifest)?;
///
/// // Write snapshots to disk
/// snapshot_plugin.write_to_dir(".bao/debug")?;
/// ```
pub struct SnapshotPlugin {
    /// Collected snapshots.
    snapshots: RwLock<Vec<PhaseSnapshot>>,
    /// Output directory for snapshots.
    output_dir: Option<PathBuf>,
}

impl SnapshotPlugin {
    /// Create a new snapshot plugin.
    pub fn new() -> Self {
        Self {
            snapshots: RwLock::new(Vec::new()),
            output_dir: None,
        }
    }

    /// Create a new snapshot plugin that writes to a directory.
    pub fn with_output_dir(output_dir: impl Into<PathBuf>) -> Self {
        Self {
            snapshots: RwLock::new(Vec::new()),
            output_dir: Some(output_dir.into()),
        }
    }

    /// Get all collected snapshots.
    pub fn snapshots(&self) -> Vec<PhaseSnapshot> {
        self.snapshots.read().unwrap().clone()
    }

    /// Write all snapshots to the configured output directory.
    pub fn write_to_dir(&self, dir: impl AsRef<Path>) -> Result<()> {
        let dir = dir.as_ref();
        fs::create_dir_all(dir)?;

        for snapshot in self.snapshots.read().unwrap().iter() {
            let filename = format!("{}.json", snapshot.phase);
            let path = dir.join(&filename);
            let json = serde_json::to_string_pretty(snapshot)?;
            fs::write(&path, json)?;
        }

        Ok(())
    }

    fn capture_snapshot(&self, phase: &str, ctx: &CompilationContext) {
        let snapshot = PhaseSnapshot {
            phase: phase.to_string(),
            ir: ctx.ir.clone(),
            computed: ctx.computed.clone(),
            diagnostics: ctx.diagnostics.clone(),
        };
        self.snapshots.write().unwrap().push(snapshot);
    }
}

impl Default for SnapshotPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for SnapshotPlugin {
    fn name(&self) -> &'static str {
        "snapshot"
    }

    fn on_after_phase(&self, phase: &str, ctx: &mut CompilationContext) -> Result<()> {
        self.capture_snapshot(phase, ctx);

        // If output directory is configured, write immediately
        if let Some(ref dir) = self.output_dir {
            let filename = format!("{}.json", phase);
            let path = dir.join(&filename);

            if let Some(snapshot) = self.snapshots.read().unwrap().last() {
                fs::create_dir_all(dir)?;
                let json = serde_json::to_string_pretty(snapshot)?;
                fs::write(&path, json)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_plugin_creation() {
        let plugin = SnapshotPlugin::new();
        assert!(plugin.snapshots().is_empty());
    }
}
