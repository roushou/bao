//! Pipeline orchestrator.

use baobao_manifest::Manifest;
use eyre::Result;

use super::{
    CompilationContext, Phase, Plugin,
    phases::{AnalyzePhase, LowerPhase, ValidatePhase},
};

/// The compilation pipeline orchestrator.
///
/// The pipeline manages the execution of compilation phases and plugin hooks.
/// It runs built-in phases (validate, lower, analyze) followed by any user
/// phases, calling plugin hooks before and after each phase.
///
/// # Example
///
/// ```ignore
/// let pipeline = Pipeline::new()
///     .plugin(MyPlugin::new())
///     .phase(MyCustomPhase);
///
/// let ctx = pipeline.run(manifest)?;
/// ```
pub struct Pipeline {
    phases: Vec<Box<dyn Phase>>,
    plugins: Vec<Box<dyn Plugin>>,
}

impl Pipeline {
    /// Create a new pipeline with default built-in phases.
    pub fn new() -> Self {
        Self {
            phases: Vec::new(),
            plugins: Vec::new(),
        }
    }

    /// Add a phase to run after the built-in phases.
    pub fn phase(mut self, phase: impl Phase + 'static) -> Self {
        self.phases.push(Box::new(phase));
        self
    }

    /// Add a plugin to receive phase lifecycle hooks.
    pub fn plugin(mut self, plugin: impl Plugin + 'static) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    /// Run the pipeline on a manifest.
    ///
    /// Executes all phases in order:
    /// 1. ValidatePhase - validates manifest, collects diagnostics
    /// 2. LowerPhase - transforms manifest to IR
    /// 3. AnalyzePhase - computes shared data
    /// 4. User phases (if any)
    ///
    /// Plugin hooks are called before and after each phase.
    ///
    /// # Errors
    ///
    /// Returns an error if any phase fails fatally.
    pub fn run(&self, manifest: Manifest) -> Result<CompilationContext> {
        let mut ctx = CompilationContext::new(manifest);

        // Built-in phases in execution order
        let builtin_phases: Vec<Box<dyn Phase>> = vec![
            Box::new(ValidatePhase::new()),
            Box::new(LowerPhase),
            Box::new(AnalyzePhase),
        ];

        // Run built-in phases, then user phases
        for phase in builtin_phases.iter().chain(self.phases.iter()) {
            self.run_phase(phase.as_ref(), &mut ctx)?;
        }

        Ok(ctx)
    }

    /// Run a single phase with plugin hooks.
    fn run_phase(&self, phase: &dyn Phase, ctx: &mut CompilationContext) -> Result<()> {
        let phase_name = phase.name();

        // Call before hooks
        for plugin in &self.plugins {
            plugin.on_before_phase(phase_name, ctx)?;
        }

        // Run the phase
        phase.run(ctx)?;

        // Call after hooks
        for plugin in &self.plugins {
            plugin.on_after_phase(phase_name, ctx)?;
        }

        Ok(())
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;

    struct CountingPlugin {
        before_count: Arc<AtomicUsize>,
        after_count: Arc<AtomicUsize>,
    }

    impl CountingPlugin {
        fn new() -> (Self, Arc<AtomicUsize>, Arc<AtomicUsize>) {
            let before = Arc::new(AtomicUsize::new(0));
            let after = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    before_count: before.clone(),
                    after_count: after.clone(),
                },
                before,
                after,
            )
        }
    }

    impl Plugin for CountingPlugin {
        fn name(&self) -> &'static str {
            "counting"
        }

        fn on_before_phase(&self, _phase: &str, _ctx: &mut CompilationContext) -> Result<()> {
            self.before_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn on_after_phase(&self, _phase: &str, _ctx: &mut CompilationContext) -> Result<()> {
            self.after_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn parse_manifest(content: &str) -> Manifest {
        toml::from_str(content).expect("Failed to parse test manifest")
    }

    fn make_test_manifest() -> Manifest {
        parse_manifest(
            r#"
            [cli]
            name = "test"
            language = "rust"
        "#,
        )
    }

    #[test]
    fn test_pipeline_runs_phases() {
        let manifest = make_test_manifest();
        let pipeline = Pipeline::new();

        let ctx = pipeline.run(manifest).expect("pipeline should succeed");

        // After running, IR and computed data should be populated
        assert!(ctx.ir.is_some());
        assert!(ctx.computed.is_some());
    }

    #[test]
    fn test_pipeline_plugin_hooks() {
        let manifest = make_test_manifest();
        let (plugin, before_count, after_count) = CountingPlugin::new();

        let pipeline = Pipeline::new().plugin(plugin);
        let _ = pipeline.run(manifest).expect("pipeline should succeed");

        // 3 built-in phases = 3 before + 3 after hooks
        assert_eq!(before_count.load(Ordering::SeqCst), 3);
        assert_eq!(after_count.load(Ordering::SeqCst), 3);
    }
}
