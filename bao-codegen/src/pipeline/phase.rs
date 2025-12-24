//! Pipeline phase trait.

use eyre::Result;

use super::CompilationContext;

/// Information about a pipeline phase.
#[derive(Debug, Clone)]
pub struct PhaseInfo {
    /// The phase name.
    pub name: &'static str,
    /// A human-readable description.
    pub description: &'static str,
}

/// A phase in the compilation pipeline.
///
/// Phases are executed in order by the pipeline. Each phase can read and
/// modify the compilation context, adding to the IR, computed data, or
/// diagnostics.
///
/// Built-in phases:
/// - `ValidatePhase` - validates the manifest and collects diagnostics
/// - `LowerPhase` - transforms manifest to Application IR
/// - `AnalyzePhase` - computes shared data from IR
///
/// Custom phases can be added to the pipeline for additional processing.
pub trait Phase: Send + Sync {
    /// The name of this phase (used in diagnostics and plugin hooks).
    fn name(&self) -> &'static str;

    /// A human-readable description of what this phase does.
    fn description(&self) -> &'static str;

    /// Run this phase on the compilation context.
    ///
    /// # Errors
    ///
    /// Returns an error if the phase fails fatally. Non-fatal issues should
    /// be recorded as diagnostics instead.
    fn run(&self, ctx: &mut CompilationContext) -> Result<()>;

    /// Get information about this phase.
    fn info(&self) -> PhaseInfo {
        PhaseInfo {
            name: self.name(),
            description: self.description(),
        }
    }
}
