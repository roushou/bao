use std::path::{Path, PathBuf};

use baobao_core::{FileRules, GeneratedFile, Overwrite, to_pascal_case, to_snake_case};

use crate::{Fn, Param, RustFile, Use};

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

    fn build_run_fn(&self) -> Fn {
        let pascal = to_pascal_case(&self.command);

        let mut f = Fn::new("run")
            .param(Param::new("_ctx", "&Context"))
            .param(Param::new("args", format!("{}Args", pascal)))
            .returns("eyre::Result<()>")
            .body_line(format!("todo!(\"implement {} command\")", self.command));

        if self.is_async {
            f = f.async_();
        }

        f
    }
}

impl GeneratedFile for HandlerStub {
    fn path(&self, base: &Path) -> PathBuf {
        // Use snake_case for file names (handles dashed names like "my-command" -> "my_command")
        let file_name = to_snake_case(&self.command);
        base.join(format!("{}.rs", file_name))
    }

    fn rules(&self) -> FileRules {
        FileRules {
            overwrite: Overwrite::IfMissing,
            header: None,
        }
    }

    fn render(&self) -> String {
        RustFile::new()
            .use_stmt(Use::new("crate::context").symbol("Context"))
            .use_stmt(Use::new(&self.args_import))
            .add(self.build_run_fn())
            .render()
    }
}
