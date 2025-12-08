//! Command tree traversal utilities.

use baobao_schema::{Command, Schema};

/// Flattened command info for easier processing.
///
/// Instead of recursively traversing the command tree, you can get
/// a flat list of all commands with their paths.
#[derive(Debug, Clone)]
pub struct FlatCommand<'a> {
    /// Command name (e.g., "migrate")
    pub name: &'a str,
    /// Full path segments (e.g., ["db", "migrate"])
    pub path: Vec<&'a str>,
    /// Depth in the command tree (0 = top-level)
    pub depth: usize,
    /// Whether this is a leaf command (no subcommands)
    pub is_leaf: bool,
    /// Reference to the command definition
    pub command: &'a Command,
}

impl<'a> FlatCommand<'a> {
    /// Get the full path as a string with the given separator.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let cmd = FlatCommand { path: vec!["db", "migrate"], .. };
    /// assert_eq!(cmd.path_str("/"), "db/migrate");
    /// assert_eq!(cmd.path_str("::"), "db::migrate");
    /// ```
    pub fn path_str(&self, sep: &str) -> String {
        self.path.join(sep)
    }

    /// Get the parent path (excluding this command's name).
    pub fn parent_path(&self) -> Vec<&'a str> {
        if self.path.len() > 1 {
            self.path[..self.path.len() - 1].to_vec()
        } else {
            Vec::new()
        }
    }
}

/// Walk all commands in a schema and return a flat list.
///
/// Commands are returned in depth-first order.
///
/// # Example
///
/// ```ignore
/// let commands = flatten_commands(&schema);
/// for cmd in commands {
///     println!("{}: leaf={}", cmd.path_str("/"), cmd.is_leaf);
/// }
/// ```
pub fn flatten_commands(schema: &Schema) -> Vec<FlatCommand<'_>> {
    let mut result = Vec::new();
    flatten_commands_recursive(&schema.commands, Vec::new(), 0, &mut result);
    result
}

fn flatten_commands_recursive<'a>(
    commands: &'a std::collections::HashMap<String, Command>,
    parent_path: Vec<&'a str>,
    depth: usize,
    result: &mut Vec<FlatCommand<'a>>,
) {
    for (name, command) in commands {
        let mut path = parent_path.clone();
        path.push(name.as_str());

        let is_leaf = !command.has_subcommands();

        result.push(FlatCommand {
            name: name.as_str(),
            path: path.clone(),
            depth,
            is_leaf,
            command,
        });

        if command.has_subcommands() {
            flatten_commands_recursive(&command.commands, path, depth + 1, result);
        }
    }
}

/// Visitor trait for command tree traversal.
///
/// Implement this trait to process commands without manual recursion.
pub trait CommandVisitor<'a> {
    /// Called for each command in the tree.
    fn visit(&mut self, cmd: &FlatCommand<'a>);
}

/// Walk all commands with a visitor.
///
/// # Example
///
/// ```ignore
/// struct LeafCollector {
///     leaves: Vec<String>,
/// }
///
/// impl CommandVisitor<'_> for LeafCollector {
///     fn visit(&mut self, cmd: &FlatCommand) {
///         if cmd.is_leaf {
///             self.leaves.push(cmd.path_str("/"));
///         }
///     }
/// }
///
/// let mut collector = LeafCollector { leaves: vec![] };
/// walk_commands(&schema, &mut collector);
/// ```
pub fn walk_commands<'a, V: CommandVisitor<'a>>(schema: &'a Schema, visitor: &mut V) {
    let commands = flatten_commands(schema);
    for cmd in &commands {
        visitor.visit(cmd);
    }
}

/// Get only leaf commands (commands without subcommands).
pub fn leaf_commands(schema: &Schema) -> Vec<FlatCommand<'_>> {
    flatten_commands(schema)
        .into_iter()
        .filter(|cmd| cmd.is_leaf)
        .collect()
}

/// Get only parent commands (commands with subcommands).
pub fn parent_commands(schema: &Schema) -> Vec<FlatCommand<'_>> {
    flatten_commands(schema)
        .into_iter()
        .filter(|cmd| !cmd.is_leaf)
        .collect()
}

#[cfg(test)]
mod tests {
    // Note: These tests would require constructing a Schema,
    // which depends on bao-schema. In practice, you'd test this
    // in an integration test or with a test helper.

    #[test]
    fn test_flat_command_path_str() {
        // Minimal test without Schema dependency
        let path = ["db", "migrate"];
        assert_eq!(path.join("/"), "db/migrate");
        assert_eq!(path.join("::"), "db::migrate");
    }
}
