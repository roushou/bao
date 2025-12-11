//! index.ts entry point generator.

use std::path::{Path, PathBuf};

use baobao_core::{FileRules, GeneratedFile, Overwrite};

use crate::{
    ast::Import,
    code_file::{CodeFile, RawCode},
};

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
        CodeFile::new()
            .add(RawCode::new("#!/usr/bin/env bun"))
            .import(Import::new("./cli.ts").named("app"))
            .add(RawCode::new("app.run();"))
            .render()
    }
}
