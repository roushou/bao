//! Command tree traversal utilities.
//!
//! This module provides the [`CommandTree`] abstraction for traversing and
//! querying commands defined in a schema.
//!
//! # Example
//!
//! ```ignore
//! use baobao_codegen::CommandTree;
//!
//! let tree = CommandTree::new(&schema);
//!
//! // Iterate all commands
//! for cmd in tree.iter() {
//!     println!("{}", cmd.path_str("/"));
//! }
//!
//! // Get only leaf commands (handlers)
//! for cmd in tree.leaves() {
//!     println!("handler: {}", cmd.path_str("/"));
//! }
//!
//! // Get only parent commands (subcommand groups)
//! for cmd in tree.parents() {
//!     println!("group: {}", cmd.path_str("/"));
//! }
//! ```

use baobao_manifest::{Command, Schema};

/// A traversable view of the command tree in a schema.
///
/// `CommandTree` provides a unified API for iterating over commands,
/// filtering by type (leaf vs parent), and accessing command metadata.
#[derive(Debug, Clone)]
pub struct CommandTree<'a> {
    commands: Vec<FlatCommand<'a>>,
}

impl<'a> CommandTree<'a> {
    /// Create a new CommandTree from a schema.
    pub fn new(schema: &'a Schema) -> Self {
        let mut commands = Vec::new();
        Self::flatten_recursive(&schema.commands, Vec::new(), 0, &mut commands);
        Self { commands }
    }

    fn flatten_recursive(
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
                Self::flatten_recursive(&command.commands, path, depth + 1, result);
            }
        }
    }

    /// Iterate over all commands in depth-first order.
    pub fn iter(&self) -> impl Iterator<Item = &FlatCommand<'a>> {
        self.commands.iter()
    }

    /// Iterate over leaf commands only (commands without subcommands).
    ///
    /// These are the commands that have handlers.
    pub fn leaves(&self) -> impl Iterator<Item = &FlatCommand<'a>> {
        self.commands.iter().filter(|cmd| cmd.is_leaf)
    }

    /// Iterate over parent commands only (commands with subcommands).
    ///
    /// These are command groups that contain other commands.
    pub fn parents(&self) -> impl Iterator<Item = &FlatCommand<'a>> {
        self.commands.iter().filter(|cmd| !cmd.is_leaf)
    }

    /// Get the total number of commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Check if the tree is empty.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Get the number of leaf commands.
    pub fn leaf_count(&self) -> usize {
        self.commands.iter().filter(|cmd| cmd.is_leaf).count()
    }

    /// Get the number of parent commands.
    pub fn parent_count(&self) -> usize {
        self.commands.iter().filter(|cmd| !cmd.is_leaf).count()
    }

    /// Convert to a Vec of all commands.
    pub fn to_vec(&self) -> Vec<FlatCommand<'a>> {
        self.commands.clone()
    }

    /// Collect all command paths as strings.
    ///
    /// Returns a set of path strings like "db/migrate", "hello".
    pub fn collect_paths(&self) -> std::collections::HashSet<String> {
        self.iter().map(|cmd| cmd.path_str("/")).collect()
    }

    /// Collect only leaf command paths (commands without subcommands).
    ///
    /// These are the paths that correspond to actual handler files.
    pub fn collect_leaf_paths(&self) -> std::collections::HashSet<String> {
        self.leaves().map(|cmd| cmd.path_str("/")).collect()
    }
}

impl<'a> IntoIterator for CommandTree<'a> {
    type Item = FlatCommand<'a>;
    type IntoIter = std::vec::IntoIter<FlatCommand<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.commands.into_iter()
    }
}

impl<'a> IntoIterator for &'a CommandTree<'a> {
    type Item = &'a FlatCommand<'a>;
    type IntoIter = std::slice::Iter<'a, FlatCommand<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.commands.iter()
    }
}

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
