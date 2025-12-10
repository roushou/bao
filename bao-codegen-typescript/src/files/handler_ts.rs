//! Handler stub generator for TypeScript projects.

use std::path::{Path, PathBuf};

use baobao_codegen::CodeBuilder;
use baobao_core::{FileRules, GeneratedFile, Overwrite, to_kebab_case, to_pascal_case};

use crate::ast::{Fn, Import, Param};

/// A handler stub file for a command.
pub struct HandlerTs {
    pub command: String,
    pub args_type: String,
}

impl HandlerTs {
    pub fn new(command: impl Into<String>, args_type: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args_type: args_type.into(),
        }
    }
}

impl GeneratedFile for HandlerTs {
    fn path(&self, base: &Path) -> PathBuf {
        let file_name = to_kebab_case(&self.command);
        base.join(format!("{}.ts", file_name))
    }

    fn rules(&self) -> FileRules {
        FileRules {
            overwrite: Overwrite::IfMissing,
            header: None,
        }
    }

    fn render(&self) -> String {
        let pascal = to_pascal_case(&self.command);
        let file_name = to_kebab_case(&self.command);

        let builder = CodeBuilder::typescript();

        // Imports
        let builder = Import::new("../context.ts")
            .named("Context")
            .type_only()
            .render(builder);
        let builder = Import::new(format!("../commands/{}.ts", file_name))
            .named(format!("{}Args", pascal))
            .type_only()
            .render(builder);

        // Handler function
        let builder = builder.blank();
        let handler = Fn::new("run")
            .async_()
            .param(Param::new("ctx", "Context"))
            .param(Param::new("args", format!("{}Args", pascal)))
            .returns("Promise<void>")
            .body_line(format!("// TODO: implement {} command", self.command))
            .body_line(format!(
                "console.log(\"{} command called with:\", args);",
                self.command
            ));

        handler.render(builder).build()
    }
}
