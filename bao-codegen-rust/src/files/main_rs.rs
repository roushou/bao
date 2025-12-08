use std::path::{Path, PathBuf};

use baobao_codegen::CodeBuilder;
use baobao_core::{FileRules, GeneratedFile, Overwrite};

use crate::Fn;

/// The main.rs entry point file (user-editable)
pub struct MainRs {
    pub is_async: bool,
}

impl MainRs {
    pub fn new(is_async: bool) -> Self {
        Self { is_async }
    }

    fn build_main_fn(&self) -> Fn {
        let body = if self.is_async {
            "app::run().await"
        } else {
            "app::run()"
        };

        let mut f = Fn::new("main")
            .private()
            .returns("eyre::Result<()>")
            .body(body);

        if self.is_async {
            f = f.async_().attr("tokio::main");
        }

        f
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
        let builder = CodeBuilder::rust()
            .line("mod app;")
            .line("mod context;")
            .line("mod generated;")
            .line("mod handlers;")
            .blank();

        self.build_main_fn().render(builder).build()
    }
}
