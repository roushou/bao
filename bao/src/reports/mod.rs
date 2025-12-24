//! Report data structures for commands.
//!
//! This module provides data structures that separate data collection from rendering.
//! Commands build reports, then render them to an Output target.

mod bake;
mod check;
mod clean;
mod explain;
mod info;
mod output;

pub use bake::{
    BakeReport, GenerationResult, HandlerChanges, PreviewFile, PreviewResult, WrittenResult,
};
pub use check::CheckReport;
pub use clean::CleanReport;
pub use explain::{
    AnalysisResult, ContextFieldInfo, ExplainReport, LintInfo, ManifestInfo, PhaseInfo,
};
pub use info::{ContextInfo, DatabaseInfo, HttpInfo, InfoReport, Stats};
pub use output::{Report, TerminalOutput};
