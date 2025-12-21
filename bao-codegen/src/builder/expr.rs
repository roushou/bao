//! Language-agnostic expression builders.
//!
//! This module provides generic abstractions for code generation that can be
//! rendered to any target language via the [`Renderer`] trait.
//!
//! # Core Abstractions
//!
//! - [`Value`] - Semantic values (bool, int, duration, enum variants, etc.)
//! - [`BuilderSpec`] - Declarative specification for builder/fluent API patterns
//! - [`Block`] - Scoped expressions with let bindings
//! - [`Renderer`] - Trait for language-specific rendering
//!
//! # Example
//!
//! ```ignore
//! use baobao_codegen::builder::{BuilderSpec, Value, Block, Binding};
//!
//! // Declarative builder specification
//! let spec = BuilderSpec::new("SqliteConnectOptions")
//!     .call_opt("create_if_missing", Some(Value::Bool(true)))
//!     .call_opt("journal_mode", Some(Value::EnumVariant {
//!         path: "SqliteJournalMode".into(),
//!         variant: "Wal".into(),
//!     }))
//!     .call_opt("busy_timeout", Some(Value::Duration { millis: 5000 }));
//!
//! // Renders differently per language:
//! // Rust:       SqliteConnectOptions::new().create_if_missing(true).journal_mode(SqliteJournalMode::Wal)
//! // TypeScript: new SqliteConnectOptions().createIfMissing(true).journalMode(JournalMode.Wal)
//! // Go:         sqlite.NewConnectOptions().CreateIfMissing(true).JournalMode(sqlite.JournalModeWal)
//! ```

use std::fmt;

/// A semantic value that can be rendered to any language.
///
/// Values represent the *meaning* of data, not syntax. Each language's
/// renderer decides how to format them appropriately.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Boolean value.
    Bool(bool),
    /// Integer value.
    Int(i64),
    /// Unsigned integer value.
    UInt(u64),
    /// Floating point value.
    Float(f64),
    /// String literal (will be quoted).
    String(String),
    /// Raw identifier or expression (not quoted).
    Ident(String),
    /// Duration in milliseconds (rendered language-appropriately).
    Duration {
        /// Duration in milliseconds.
        millis: u64,
    },
    /// Enum variant reference.
    EnumVariant {
        /// Full path to the enum type (e.g., "sqlx::sqlite::SqliteJournalMode").
        path: String,
        /// Variant name (e.g., "Wal").
        variant: String,
    },
    /// Nested builder specification.
    Builder(Box<BuilderSpec>),
    /// Nested block expression.
    Block(Box<Block>),
}

impl Value {
    /// Create a boolean value.
    pub fn bool(v: bool) -> Self {
        Self::Bool(v)
    }

    /// Create an integer value.
    pub fn int(v: i64) -> Self {
        Self::Int(v)
    }

    /// Create an unsigned integer value.
    pub fn uint(v: u64) -> Self {
        Self::UInt(v)
    }

    /// Create a float value.
    pub fn float(v: f64) -> Self {
        Self::Float(v)
    }

    /// Create a string literal value.
    pub fn string(v: impl Into<String>) -> Self {
        Self::String(v.into())
    }

    /// Create an identifier/expression value.
    pub fn ident(v: impl Into<String>) -> Self {
        Self::Ident(v.into())
    }

    /// Create a duration value from seconds.
    pub fn duration_secs(secs: u64) -> Self {
        Self::Duration {
            millis: secs * 1000,
        }
    }

    /// Create a duration value from milliseconds.
    pub fn duration_millis(millis: u64) -> Self {
        Self::Duration { millis }
    }

    /// Create an enum variant value.
    pub fn enum_variant(path: impl Into<String>, variant: impl Into<String>) -> Self {
        Self::EnumVariant {
            path: path.into(),
            variant: variant.into(),
        }
    }

    /// Create a nested builder value.
    pub fn builder(spec: BuilderSpec) -> Self {
        Self::Builder(Box::new(spec))
    }

    /// Create a nested block value.
    pub fn block(block: Block) -> Self {
        Self::Block(Box::new(block))
    }
}

/// How to construct the builder's base expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Constructor {
    /// Static method constructor: `Type::new()` or `Type.new()`.
    StaticNew {
        /// Full type path (e.g., "sqlx::pool::PoolOptions").
        type_path: String,
    },
    /// Class-style constructor: `new Type()`.
    ClassNew {
        /// Type name (e.g., "PoolOptions").
        type_name: String,
    },
    /// Factory function: `createType()` or `NewType()`.
    Factory {
        /// Factory function name.
        name: String,
    },
    /// Raw expression as the base.
    Raw {
        /// Raw expression string.
        expr: String,
    },
}

impl Constructor {
    /// Create a static `::new()` constructor.
    pub fn static_new(type_path: impl Into<String>) -> Self {
        Self::StaticNew {
            type_path: type_path.into(),
        }
    }

    /// Create a class-style `new` constructor.
    pub fn class_new(type_name: impl Into<String>) -> Self {
        Self::ClassNew {
            type_name: type_name.into(),
        }
    }

    /// Create a factory function constructor.
    pub fn factory(name: impl Into<String>) -> Self {
        Self::Factory { name: name.into() }
    }

    /// Create a raw expression constructor.
    pub fn raw(expr: impl Into<String>) -> Self {
        Self::Raw { expr: expr.into() }
    }
}

/// A method call in a builder chain.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodCall {
    /// Method name (in canonical snake_case form).
    pub name: String,
    /// Arguments to the method.
    pub args: Vec<Value>,
}

impl MethodCall {
    /// Create a new method call.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            args: Vec::new(),
        }
    }

    /// Add an argument to the method call.
    pub fn arg(mut self, value: Value) -> Self {
        self.args.push(value);
        self
    }

    /// Add multiple arguments.
    pub fn args(mut self, values: impl IntoIterator<Item = Value>) -> Self {
        self.args.extend(values);
        self
    }
}

/// Terminal operation for a builder chain.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Terminal {
    /// Whether to add `.await` (or language equivalent).
    pub is_async: bool,
    /// Whether to add `?` or error propagation.
    pub is_try: bool,
    /// Optional final method call (e.g., `.build()`).
    pub method: Option<String>,
}

impl Terminal {
    /// Create a new terminal with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark as async (adds `.await` in Rust, `await` in JS, etc.).
    pub fn async_(mut self) -> Self {
        self.is_async = true;
        self
    }

    /// Mark as try (adds `?` in Rust, try/catch wrapper in JS, etc.).
    pub fn try_(mut self) -> Self {
        self.is_try = true;
        self
    }

    /// Add a terminal method call.
    pub fn method(mut self, name: impl Into<String>) -> Self {
        self.method = Some(name.into());
        self
    }
}

/// A declarative specification for a builder pattern.
///
/// This represents the *intent* of building an object via method chaining,
/// independent of any specific language syntax.
#[derive(Debug, Clone, PartialEq)]
pub struct BuilderSpec {
    /// How to construct the base expression.
    pub constructor: Constructor,
    /// Method calls in order.
    pub calls: Vec<MethodCall>,
    /// Terminal operation (await, try, build method).
    pub terminal: Terminal,
}

impl BuilderSpec {
    /// Create a new builder spec with a static `::new()` constructor.
    pub fn new(type_path: impl Into<String>) -> Self {
        Self {
            constructor: Constructor::static_new(type_path),
            calls: Vec::new(),
            terminal: Terminal::default(),
        }
    }

    /// Create a builder spec with a custom constructor.
    pub fn with_constructor(constructor: Constructor) -> Self {
        Self {
            constructor,
            calls: Vec::new(),
            terminal: Terminal::default(),
        }
    }

    /// Add a method call with no arguments.
    pub fn call(mut self, name: impl Into<String>) -> Self {
        self.calls.push(MethodCall::new(name));
        self
    }

    /// Add a method call with a single argument.
    pub fn call_arg(mut self, name: impl Into<String>, value: Value) -> Self {
        self.calls.push(MethodCall::new(name).arg(value));
        self
    }

    /// Add a method call with multiple arguments.
    pub fn call_args(
        mut self,
        name: impl Into<String>,
        values: impl IntoIterator<Item = Value>,
    ) -> Self {
        self.calls.push(MethodCall::new(name).args(values));
        self
    }

    /// Conditionally add a method call.
    pub fn call_if(self, condition: bool, name: impl Into<String>) -> Self {
        if condition { self.call(name) } else { self }
    }

    /// Conditionally add a method call with argument.
    pub fn call_arg_if(self, condition: bool, name: impl Into<String>, value: Value) -> Self {
        if condition {
            self.call_arg(name, value)
        } else {
            self
        }
    }

    /// Add a method call if the value is Some.
    pub fn call_opt(self, name: impl Into<String>, value: Option<Value>) -> Self {
        match value {
            Some(v) => self.call_arg(name, v),
            None => self,
        }
    }

    /// Mark the chain as async.
    pub fn async_(mut self) -> Self {
        self.terminal.is_async = true;
        self
    }

    /// Mark the chain as try (error propagation).
    pub fn try_(mut self) -> Self {
        self.terminal.is_try = true;
        self
    }

    /// Add a terminal method call.
    pub fn terminal_method(mut self, name: impl Into<String>) -> Self {
        self.terminal.method = Some(name.into());
        self
    }

    /// Check if the spec has any method calls.
    pub fn has_calls(&self) -> bool {
        !self.calls.is_empty()
    }

    /// Apply configuration from an iterator of optional values.
    ///
    /// This enables declarative config-to-chain mapping:
    /// ```ignore
    /// spec.apply_config([
    ///     ("max_connections", config.max.map(Value::uint)),
    ///     ("timeout", config.timeout.map(Value::duration_secs)),
    /// ])
    /// ```
    pub fn apply_config<I, S>(mut self, config: I) -> Self
    where
        I: IntoIterator<Item = (S, Option<Value>)>,
        S: Into<String>,
    {
        for (name, value) in config {
            if let Some(v) = value {
                self.calls.push(MethodCall::new(name).arg(v));
            }
        }
        self
    }
}

/// A binding in a block (let/const/var).
#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    /// Variable name.
    pub name: String,
    /// Value to bind.
    pub value: Value,
    /// Whether the binding is mutable.
    pub mutable: bool,
}

impl Binding {
    /// Create a new immutable binding.
    pub fn new(name: impl Into<String>, value: Value) -> Self {
        Self {
            name: name.into(),
            value,
            mutable: false,
        }
    }

    /// Create a new mutable binding.
    pub fn new_mut(name: impl Into<String>, value: Value) -> Self {
        Self {
            name: name.into(),
            value,
            mutable: true,
        }
    }
}

/// A scoped block expression with bindings.
///
/// Represents `{ let x = ...; let y = ...; result }` patterns that exist
/// across languages (Rust blocks, JS IIFEs, Go closures).
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// Variable bindings.
    pub bindings: Vec<Binding>,
    /// The final expression/result.
    pub body: Value,
}

impl Block {
    /// Create a new block with the given body.
    pub fn new(body: Value) -> Self {
        Self {
            bindings: Vec::new(),
            body,
        }
    }

    /// Add a let binding.
    pub fn binding(mut self, name: impl Into<String>, value: Value) -> Self {
        self.bindings.push(Binding::new(name, value));
        self
    }

    /// Add a mutable binding.
    pub fn binding_mut(mut self, name: impl Into<String>, value: Value) -> Self {
        self.bindings.push(Binding::new_mut(name, value));
        self
    }
}

/// Formatting options for rendering.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Base indentation level.
    pub indent: usize,
    /// Spaces per indent level.
    pub indent_size: usize,
    /// Whether to render inline (single line) when possible.
    pub inline: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            indent: 0,
            indent_size: 4,
            inline: false,
        }
    }
}

impl RenderOptions {
    /// Create options for inline rendering.
    pub fn inline() -> Self {
        Self {
            inline: true,
            ..Default::default()
        }
    }

    /// Set the base indentation level.
    pub fn with_indent(mut self, indent: usize) -> Self {
        self.indent = indent;
        self
    }

    /// Set spaces per indent level.
    pub fn with_indent_size(mut self, size: usize) -> Self {
        self.indent_size = size;
        self
    }

    /// Get the current indentation string.
    pub fn indent_str(&self) -> String {
        " ".repeat(self.indent * self.indent_size)
    }

    /// Get indentation for a nested level.
    pub fn nested(&self) -> Self {
        Self {
            indent: self.indent + 1,
            ..*self
        }
    }
}

/// Trait for language-specific rendering of expressions.
///
/// Implement this trait to support a new target language.
pub trait Renderer {
    /// Render a value to a string.
    fn render_value(&self, value: &Value, opts: &RenderOptions) -> String;

    /// Render a builder specification to a string.
    fn render_builder(&self, spec: &BuilderSpec, opts: &RenderOptions) -> String;

    /// Render a block expression to a string.
    fn render_block(&self, block: &Block, opts: &RenderOptions) -> String;

    /// Transform a method name to the target language convention.
    ///
    /// Default implementation returns the name as-is (snake_case).
    fn transform_method_name(&self, name: &str) -> String {
        name.to_string()
    }

    /// Render a constructor to a string.
    fn render_constructor(&self, ctor: &Constructor) -> String;

    /// Render terminal operations (await, try, build method).
    fn render_terminal(&self, terminal: &Terminal) -> String;
}

/// Extension trait for convenient rendering.
pub trait RenderExt {
    /// Render using the given renderer with default options.
    fn render(&self, renderer: &dyn Renderer) -> String;

    /// Render inline using the given renderer.
    fn render_inline(&self, renderer: &dyn Renderer) -> String;

    /// Render with custom options.
    fn render_with(&self, renderer: &dyn Renderer, opts: &RenderOptions) -> String;
}

impl RenderExt for Value {
    fn render(&self, renderer: &dyn Renderer) -> String {
        renderer.render_value(self, &RenderOptions::default())
    }

    fn render_inline(&self, renderer: &dyn Renderer) -> String {
        renderer.render_value(self, &RenderOptions::inline())
    }

    fn render_with(&self, renderer: &dyn Renderer, opts: &RenderOptions) -> String {
        renderer.render_value(self, opts)
    }
}

impl RenderExt for BuilderSpec {
    fn render(&self, renderer: &dyn Renderer) -> String {
        renderer.render_builder(self, &RenderOptions::default())
    }

    fn render_inline(&self, renderer: &dyn Renderer) -> String {
        renderer.render_builder(self, &RenderOptions::inline())
    }

    fn render_with(&self, renderer: &dyn Renderer, opts: &RenderOptions) -> String {
        renderer.render_builder(self, opts)
    }
}

impl RenderExt for Block {
    fn render(&self, renderer: &dyn Renderer) -> String {
        renderer.render_block(self, &RenderOptions::default())
    }

    fn render_inline(&self, renderer: &dyn Renderer) -> String {
        renderer.render_block(self, &RenderOptions::inline())
    }

    fn render_with(&self, renderer: &dyn Renderer, opts: &RenderOptions) -> String {
        renderer.render_block(self, opts)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Debug representation for display
        match self {
            Value::Bool(v) => write!(f, "{}", v),
            Value::Int(v) => write!(f, "{}", v),
            Value::UInt(v) => write!(f, "{}", v),
            Value::Float(v) => write!(f, "{}", v),
            Value::String(v) => write!(f, "\"{}\"", v),
            Value::Ident(v) => write!(f, "{}", v),
            Value::Duration { millis } => write!(f, "{}ms", millis),
            Value::EnumVariant { path, variant } => write!(f, "{}::{}", path, variant),
            Value::Builder(_) => write!(f, "<builder>"),
            Value::Block(_) => write!(f, "<block>"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_constructors() {
        assert_eq!(Value::bool(true), Value::Bool(true));
        assert_eq!(Value::int(42), Value::Int(42));
        assert_eq!(Value::uint(100), Value::UInt(100));
        assert_eq!(Value::string("hello"), Value::String("hello".into()));
        assert_eq!(Value::ident("foo"), Value::Ident("foo".into()));
        assert_eq!(Value::duration_secs(5), Value::Duration { millis: 5000 });
        assert_eq!(Value::duration_millis(100), Value::Duration { millis: 100 });
    }

    #[test]
    fn test_builder_spec_basic() {
        let spec = BuilderSpec::new("PoolOptions")
            .call_arg("max_connections", Value::uint(10))
            .call_arg("min_connections", Value::uint(5));

        assert_eq!(spec.calls.len(), 2);
        assert_eq!(spec.calls[0].name, "max_connections");
        assert_eq!(spec.calls[1].name, "min_connections");
    }

    #[test]
    fn test_builder_spec_conditional() {
        let spec = BuilderSpec::new("Options")
            .call_opt("present", Some(Value::bool(true)))
            .call_opt("missing", None)
            .call_if(true, "enabled")
            .call_if(false, "disabled");

        assert_eq!(spec.calls.len(), 2);
        assert_eq!(spec.calls[0].name, "present");
        assert_eq!(spec.calls[1].name, "enabled");
    }

    #[test]
    fn test_builder_spec_apply_config() {
        let max: Option<u64> = Some(10);
        let min: Option<u64> = None;
        let timeout: Option<u64> = Some(30);

        let spec = BuilderSpec::new("PoolOptions").apply_config([
            ("max_connections", max.map(Value::uint)),
            ("min_connections", min.map(Value::uint)),
            ("timeout", timeout.map(Value::duration_secs)),
        ]);

        assert_eq!(spec.calls.len(), 2);
        assert_eq!(spec.calls[0].name, "max_connections");
        assert_eq!(spec.calls[1].name, "timeout");
    }

    #[test]
    fn test_block_with_bindings() {
        let block = Block::new(Value::ident("pool"))
            .binding("options", Value::builder(BuilderSpec::new("SqliteOptions")))
            .binding_mut("counter", Value::int(0));

        assert_eq!(block.bindings.len(), 2);
        assert!(!block.bindings[0].mutable);
        assert!(block.bindings[1].mutable);
    }

    #[test]
    fn test_constructor_variants() {
        let static_new = Constructor::static_new("sqlx::pool::PoolOptions");
        let class_new = Constructor::class_new("PoolOptions");
        let factory = Constructor::factory("NewPoolOptions");
        let raw = Constructor::raw("get_options()");

        assert!(matches!(static_new, Constructor::StaticNew { .. }));
        assert!(matches!(class_new, Constructor::ClassNew { .. }));
        assert!(matches!(factory, Constructor::Factory { .. }));
        assert!(matches!(raw, Constructor::Raw { .. }));
    }

    #[test]
    fn test_terminal_operations() {
        let terminal = Terminal::new().async_().try_().method("build");

        assert!(terminal.is_async);
        assert!(terminal.is_try);
        assert_eq!(terminal.method, Some("build".into()));
    }

    #[test]
    fn test_render_options() {
        let opts = RenderOptions::default().with_indent(2).with_indent_size(2);
        assert_eq!(opts.indent_str(), "    ");

        let nested = opts.nested();
        assert_eq!(nested.indent_str(), "      ");
    }
}
