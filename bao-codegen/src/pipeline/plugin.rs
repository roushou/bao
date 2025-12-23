//! Pipeline plugin trait for extensibility.

use eyre::Result;

use super::CompilationContext;

/// A plugin that can hook into the compilation pipeline.
///
/// Plugins receive callbacks before and after each phase runs, allowing
/// them to inspect or modify the compilation context.
///
/// # Example
///
/// ```ignore
/// struct TimingPlugin {
///     start_times: RefCell<HashMap<String, Instant>>,
/// }
///
/// impl Plugin for TimingPlugin {
///     fn name(&self) -> &'static str { "timing" }
///
///     fn on_before_phase(&self, phase: &str, _ctx: &mut CompilationContext) -> Result<()> {
///         self.start_times.borrow_mut().insert(phase.to_string(), Instant::now());
///         Ok(())
///     }
///
///     fn on_after_phase(&self, phase: &str, _ctx: &mut CompilationContext) -> Result<()> {
///         if let Some(start) = self.start_times.borrow().get(phase) {
///             println!("{} took {:?}", phase, start.elapsed());
///         }
///         Ok(())
///     }
/// }
/// ```
pub trait Plugin: Send + Sync {
    /// The name of this plugin (for debugging and logging).
    fn name(&self) -> &'static str;

    /// Called before a phase runs.
    ///
    /// # Arguments
    ///
    /// * `phase` - The name of the phase about to run
    /// * `ctx` - The compilation context (can be modified)
    ///
    /// # Errors
    ///
    /// Return an error to abort the pipeline.
    #[allow(unused_variables)]
    fn on_before_phase(&self, phase: &str, ctx: &mut CompilationContext) -> Result<()> {
        Ok(())
    }

    /// Called after a phase completes successfully.
    ///
    /// # Arguments
    ///
    /// * `phase` - The name of the phase that just completed
    /// * `ctx` - The compilation context (can be modified)
    ///
    /// # Errors
    ///
    /// Return an error to abort the pipeline.
    #[allow(unused_variables)]
    fn on_after_phase(&self, phase: &str, ctx: &mut CompilationContext) -> Result<()> {
        Ok(())
    }
}
