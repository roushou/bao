use std::path::{Path, PathBuf};

use baobao_core::{FileRules, GeneratedFile, Overwrite};

/// The main.rs entry point file (user-editable)
pub struct MainRs {
    pub is_async: bool,
}

impl MainRs {
    pub fn new(is_async: bool) -> Self {
        Self { is_async }
    }
}

impl GeneratedFile for MainRs {
    fn path(&self, base: &Path) -> PathBuf {
        base.join("src").join("main.rs")
    }

    fn rules(&self) -> FileRules {
        FileRules {
            overwrite: Overwrite::IfMissing,
            header: None,
        }
    }

    fn render(&self) -> String {
        if self.is_async {
            r#"mod app;
mod context;
mod generated;
mod handlers;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    app::run().await
}
"#
            .to_string()
        } else {
            r#"mod app;
mod context;
mod generated;
mod handlers;

fn main() -> eyre::Result<()> {
    app::run()
}
"#
            .to_string()
        }
    }
}
