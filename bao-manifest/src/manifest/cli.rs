use baobao_core::Version;
use serde::Deserialize;

use super::Language;

/// CLI metadata configuration
#[derive(Debug, Deserialize)]
pub struct CliConfig {
    /// Name of the CLI binary
    pub name: String,

    /// Version
    #[serde(default = "default_version")]
    pub version: Version,

    /// CLI description for help text
    pub description: Option<String>,

    /// Author information
    pub author: Option<String>,

    /// Target language for code generation
    pub language: Language,
}

fn default_version() -> Version {
    Version::new(0, 1, 0)
}
