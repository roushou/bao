//! index.ts entry point generator.

use std::path::{Path, PathBuf};

use baobao_core::{FileRules, GeneratedFile};

use crate::{
    Shebang,
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
        FileRules::create_once()
    }

    fn render(&self) -> String {
        CodeFile::new()
            .add(Shebang::bun())
            .import(Import::new("./cli.ts").named("app"))
            .add(RawCode::new("app.run();"))
            .render()
    }
}
