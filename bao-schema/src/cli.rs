use serde::Deserialize;

/// CLI metadata configuration
#[derive(Debug, Deserialize)]
pub struct CliConfig {
    /// Name of the CLI binary
    pub name: String,

    /// Version string
    #[serde(default = "default_version")]
    pub version: String,

    /// CLI description for help text
    pub description: Option<String>,

    /// Author information
    pub author: Option<String>,
}

fn default_version() -> String {
    "0.1.0".to_string()
}
