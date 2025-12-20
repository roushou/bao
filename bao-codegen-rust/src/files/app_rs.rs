use std::path::{Path, PathBuf};

use baobao_core::{FileRules, GeneratedFile};

use super::{GENERATED_HEADER, uses};
use crate::{Fn, RustFile, Use};

/// The app.rs file that handles Context setup and CLI dispatch
pub struct AppRs {
    pub is_async: bool,
}

impl AppRs {
    pub fn new(is_async: bool) -> Self {
        Self { is_async }
    }

    fn build_run_fn(&self) -> Fn {
        let await_suffix = if self.is_async { ".await" } else { "" };
        let body = format!(
            "let ctx = Context::new(){}?;\nCli::parse().dispatch(&ctx){}",
            await_suffix, await_suffix
        );

        Fn::new("run")
            .returns("eyre::Result<()>")
            .body(body)
            .async_if(self.is_async)
    }
}

impl GeneratedFile for AppRs {
    fn path(&self, base: &Path) -> PathBuf {
        base.join("src").join("app.rs")
    }

    fn rules(&self) -> FileRules {
        FileRules::always_overwrite().with_header(GENERATED_HEADER)
    }

    fn render(&self) -> String {
        RustFile::new()
            .use_stmt(uses::clap_parser())
            .use_stmt(uses::context())
            .use_stmt(Use::new("crate::generated").symbol("Cli"))
            .add(self.build_run_fn())
            .render_with_header(GENERATED_HEADER)
    }
}
