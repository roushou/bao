//! Handler stub generator for TypeScript projects.

use std::path::{Path, PathBuf};

use baobao_codegen::CodeBuilder;
use baobao_core::{FileRules, GeneratedFile, Overwrite, to_kebab_case, to_pascal_case};

use crate::ast::{Fn, Import, Param};

/// A handler stub file for a command.
pub struct HandlerTs {
    pub command: String,
    /// Path segments for nested handlers (e.g., ["data", "builders", "leaderboard"])
    pub path_segments: Vec<String>,
    /// Whether the command has arguments
    pub has_args: bool,
    /// Whether the command has options/flags
    pub has_options: bool,
}

impl HandlerTs {
    pub fn new(command: impl Into<String>) -> Self {
        let cmd = command.into();
        Self {
            command: cmd.clone(),
            path_segments: vec![cmd],
            has_args: true,
            has_options: false,
        }
    }

    /// Create a handler at a nested path.
    pub fn nested(
        command: impl Into<String>,
        path_segments: Vec<String>,
        has_args: bool,
        has_options: bool,
    ) -> Self {
        Self {
            command: command.into(),
            path_segments,
            has_args,
            has_options,
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

        // Build the command file path (kebab-case, joined by /)
        let command_path = self
            .path_segments
            .iter()
            .map(|s| to_kebab_case(s))
            .collect::<Vec<_>>()
            .join("/");

        // Calculate relative path based on depth
        let depth = self.path_segments.len();
        let up_path = "../".repeat(depth);

        let builder = CodeBuilder::typescript();

        // Import the exported types from the command file
        let mut import = Import::new(format!("{}commands/{}.ts", up_path, command_path));
        if self.has_args {
            import = import.named_type(format!("{}Args", pascal));
        }
        if self.has_options {
            import = import.named_type(format!("{}Options", pascal));
        }
        let builder = import.render(builder);

        // Handler function with imported types
        let builder = builder.blank();
        let mut handler = Fn::new("run").async_();

        if self.has_args {
            handler = handler.param(Param::new("args", format!("{}Args", pascal)));
        }
        if self.has_options {
            handler = handler.param(Param::new("options", format!("{}Options", pascal)));
        }

        // Build console.log based on what's available
        let log_args = match (self.has_args, self.has_options) {
            (true, true) => "console.log(args, options);",
            (true, false) => "console.log(args);",
            (false, true) => "console.log(options);",
            (false, false) => "// no args or options",
        };

        handler = handler
            .returns("Promise<void>")
            .body_line(format!("// TODO: implement {} command", self.command))
            .body_line(log_args);

        handler.render(builder).build()
    }
}
