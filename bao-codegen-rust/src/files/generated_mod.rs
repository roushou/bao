use std::path::{Path, PathBuf};

use baobao_core::{FileRules, GeneratedFile};

use super::GENERATED_HEADER;
use crate::{RawCode, RustFile};

/// The generated/mod.rs file that exports the CLI and commands
pub struct GeneratedMod;

impl GeneratedFile for GeneratedMod {
    fn path(&self, base: &Path) -> PathBuf {
        base.join("src").join("generated").join("mod.rs")
    }

    fn rules(&self) -> FileRules {
        FileRules::always_overwrite().with_header(GENERATED_HEADER)
    }

    fn render(&self) -> String {
        RustFile::new()
            .add(RawCode::lines(["pub mod cli;", "pub mod commands;"]))
            .add(RawCode::new("pub use cli::*;"))
            .render_with_header(GENERATED_HEADER)
    }
}
