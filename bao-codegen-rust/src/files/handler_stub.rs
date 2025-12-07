use std::path::{Path, PathBuf};

use baobao_core::{FileRules, GeneratedFile, Overwrite};

/// A handler stub file for a command
pub struct HandlerStub {
    pub command: String,
    pub args_import: String,
    pub is_async: bool,
}

impl HandlerStub {
    pub fn new(command: impl Into<String>, args_import: impl Into<String>, is_async: bool) -> Self {
        Self {
            command: command.into(),
            args_import: args_import.into(),
            is_async,
        }
    }
}

impl GeneratedFile for HandlerStub {
    fn path(&self, base: &Path) -> PathBuf {
        base.join(format!("{}.rs", self.command))
    }

    fn rules(&self) -> FileRules {
        FileRules {
            overwrite: Overwrite::IfMissing,
            header: None,
        }
    }

    fn render(&self) -> String {
        let pascal = to_pascal_case(&self.command);
        let async_kw = if self.is_async { "async " } else { "" };

        format!(
            r#"use crate::context::Context;
use {};

pub {}fn run(_ctx: &Context, args: {}Args) -> eyre::Result<()> {{
    todo!("implement {} command")
}}
"#,
            self.args_import, async_kw, pascal, self.command
        )
    }
}

fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}
