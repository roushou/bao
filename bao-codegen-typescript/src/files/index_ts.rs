//! index.ts entry point generator.

use std::path::{Path, PathBuf};

use baobao_codegen::CodeBuilder;
use baobao_core::{FileRules, GeneratedFile, Overwrite};

use crate::ast::Import;

/// The index.ts entry point file.
pub struct IndexTs;

impl GeneratedFile for IndexTs {
    fn path(&self, base: &Path) -> PathBuf {
        base.join("src").join("index.ts")
    }

    fn rules(&self) -> FileRules {
        FileRules {
            overwrite: Overwrite::IfMissing,
            header: None,
        }
    }

    fn render(&self) -> String {
        let builder = CodeBuilder::typescript().line("#!/usr/bin/env bun");

        let builder = Import::new("./cli.ts").named("app").render(builder);
        let builder = builder.blank().line("app.run();");

        builder.build()
    }
}
