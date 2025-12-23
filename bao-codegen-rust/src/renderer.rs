//! Rust-specific renderer for language-agnostic expressions.
//!
//! This module implements the [`Renderer`] trait for Rust, translating
//! [`Value`], [`BuilderSpec`], and [`Block`] into valid Rust syntax.

use baobao_codegen::builder::{
    Binding, Block, BuilderSpec, Constructor, RenderOptions, Renderer, Terminal, Value,
};

/// Rust language renderer.
///
/// Renders language-agnostic expressions to valid Rust syntax.
///
/// # Examples
///
/// ```
/// use baobao_codegen::builder::{BuilderSpec, Value};
/// use baobao_codegen_rust::RustRenderer;
///
/// let spec = BuilderSpec::new("sqlx::pool::PoolOptions")
///     .call_arg("max_connections", Value::uint(10))
///     .call_arg("min_connections", Value::uint(5))
///     .async_()
///     .try_();
///
/// let rust = RustRenderer;
/// let code = spec.render(&rust);
/// // Produces:
/// // sqlx::pool::PoolOptions::new()
/// //     .max_connections(10)
/// //     .min_connections(5)
/// //     .await?
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct RustRenderer;

impl RustRenderer {
    /// Create a new Rust renderer.
    pub fn new() -> Self {
        Self
    }

    /// Render a binding to Rust syntax.
    fn render_binding(&self, binding: &Binding, opts: &RenderOptions) -> String {
        let indent = opts.indent_str();
        let mutability = if binding.mutable { "mut " } else { "" };
        let value = self.render_value(&binding.value, &opts.nested());
        format!("{}let {}{} = {};", indent, mutability, binding.name, value)
    }
}

impl Renderer for RustRenderer {
    fn render_value(&self, value: &Value, opts: &RenderOptions) -> String {
        match value {
            Value::Bool(v) => v.to_string(),
            Value::Int(v) => v.to_string(),
            Value::UInt(v) => v.to_string(),
            Value::Float(v) => {
                let s = v.to_string();
                // Ensure float has decimal point
                if s.contains('.') {
                    s
                } else {
                    format!("{}.0", s)
                }
            }
            Value::String(v) => format!("\"{}\"", v),
            Value::Ident(v) => v.clone(),
            Value::Duration { millis } => {
                // Choose appropriate unit for readability
                if *millis >= 1000 && millis % 1000 == 0 {
                    format!("std::time::Duration::from_secs({})", millis / 1000)
                } else {
                    format!("std::time::Duration::from_millis({})", millis)
                }
            }
            Value::EnumVariant { path, variant } => {
                format!("{}::{}", path, variant)
            }
            Value::EnvVar { name, by_ref } => {
                let prefix = if *by_ref { "&" } else { "" };
                format!("{}std::env::var(\"{}\")?", prefix, name)
            }
            Value::Try(inner) => {
                format!("{}?", self.render_value(inner, opts))
            }
            Value::Builder(spec) => self.render_builder(spec, opts),
            Value::Block(block) => self.render_block(block, opts),
        }
    }

    fn render_builder(&self, spec: &BuilderSpec, opts: &RenderOptions) -> String {
        let mut result = self.render_constructor(&spec.constructor);

        if spec.calls.is_empty() {
            result.push_str(&self.render_terminal(&spec.terminal));
            return result;
        }

        if opts.inline {
            // Single line format
            for call in &spec.calls {
                let name = self.transform_method_name(&call.name);
                if call.args.is_empty() {
                    result.push_str(&format!(".{}()", name));
                } else {
                    let args: Vec<String> = call
                        .args
                        .iter()
                        .map(|a| self.render_value(a, opts))
                        .collect();
                    result.push_str(&format!(".{}({})", name, args.join(", ")));
                }
            }
        } else {
            // Multi-line format: continuation lines are indented one level deeper
            let continuation = opts.nested();
            let indent = continuation.indent_str();
            for call in &spec.calls {
                let name = self.transform_method_name(&call.name);
                if call.args.is_empty() {
                    result.push_str(&format!("\n{}.{}()", indent, name));
                } else {
                    let args: Vec<String> = call
                        .args
                        .iter()
                        .map(|a| self.render_value(a, &continuation))
                        .collect();
                    result.push_str(&format!("\n{}.{}({})", indent, name, args.join(", ")));
                }
            }
        }

        result.push_str(&self.render_terminal(&spec.terminal));
        result
    }

    fn render_block(&self, block: &Block, opts: &RenderOptions) -> String {
        if block.bindings.is_empty() {
            // No bindings, just render the body
            return self.render_value(&block.body, opts);
        }

        let indent = opts.indent_str();
        let inner_opts = opts.nested();
        let inner_indent = inner_opts.indent_str();

        let mut result = String::from("{\n");

        // Render bindings
        for binding in &block.bindings {
            result.push_str(&self.render_binding(binding, &inner_opts));
            result.push('\n');
        }

        // Render body
        result.push_str(&inner_indent);
        result.push_str(&self.render_value(&block.body, &inner_opts));
        result.push('\n');

        result.push_str(&indent);
        result.push('}');

        result
    }

    fn transform_method_name(&self, name: &str) -> String {
        // Rust uses snake_case, which is our canonical form
        name.to_string()
    }

    fn render_constructor(&self, ctor: &Constructor) -> String {
        match ctor {
            Constructor::StaticNew { type_path } => {
                format!("{}::new()", type_path)
            }
            Constructor::StaticMethod {
                type_path,
                method,
                args,
            } => {
                let opts = RenderOptions::inline();
                let rendered_args: Vec<String> =
                    args.iter().map(|a| self.render_value(a, &opts)).collect();
                format!("{}::{}({})", type_path, method, rendered_args.join(", "))
            }
            Constructor::ClassNew { type_name } => {
                // Rust doesn't have class-style new, use static new
                format!("{}::new()", type_name)
            }
            Constructor::Factory { name } => {
                format!("{}()", name)
            }
        }
    }

    fn render_terminal(&self, terminal: &Terminal) -> String {
        let mut result = String::new();

        if let Some(method) = &terminal.method {
            result.push_str(&format!(".{}()", method));
        }

        if terminal.is_async {
            result.push_str(".await");
        }

        if terminal.is_try {
            result.push('?');
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_values() {
        let r = RustRenderer;
        let opts = RenderOptions::inline();

        assert_eq!(r.render_value(&Value::Bool(true), &opts), "true");
        assert_eq!(r.render_value(&Value::Int(-42), &opts), "-42");
        assert_eq!(r.render_value(&Value::UInt(100), &opts), "100");
        assert_eq!(
            r.render_value(&Value::String("hello".into()), &opts),
            "\"hello\""
        );
        assert_eq!(r.render_value(&Value::Ident("foo".into()), &opts), "foo");
    }

    #[test]
    fn test_render_duration() {
        let r = RustRenderer;
        let opts = RenderOptions::inline();

        // Seconds (clean division)
        assert_eq!(
            r.render_value(&Value::Duration { millis: 5000 }, &opts),
            "std::time::Duration::from_secs(5)"
        );

        // Milliseconds (not evenly divisible)
        assert_eq!(
            r.render_value(&Value::Duration { millis: 1500 }, &opts),
            "std::time::Duration::from_millis(1500)"
        );

        // Sub-second
        assert_eq!(
            r.render_value(&Value::Duration { millis: 100 }, &opts),
            "std::time::Duration::from_millis(100)"
        );
    }

    #[test]
    fn test_render_enum_variant() {
        let r = RustRenderer;
        let opts = RenderOptions::inline();

        let value = Value::EnumVariant {
            path: "sqlx::sqlite::SqliteJournalMode".into(),
            variant: "Wal".into(),
        };
        assert_eq!(
            r.render_value(&value, &opts),
            "sqlx::sqlite::SqliteJournalMode::Wal"
        );
    }

    #[test]
    fn test_render_builder_inline() {
        let r = RustRenderer;

        let spec = BuilderSpec::new("PoolOptions")
            .call_arg("max_connections", Value::uint(10))
            .call_arg("min_connections", Value::uint(5));

        assert_eq!(
            spec.render_inline(&r),
            "PoolOptions::new().max_connections(10).min_connections(5)"
        );
    }

    #[test]
    fn test_render_builder_multiline() {
        let r = RustRenderer;

        let spec = BuilderSpec::new("PoolOptions")
            .call_arg("max_connections", Value::uint(10))
            .call_arg("min_connections", Value::uint(5))
            .async_()
            .try_();

        let result = spec.render(&r);
        assert!(result.contains("PoolOptions::new()"));
        assert!(result.contains("\n    .max_connections(10)"));
        assert!(result.contains("\n    .min_connections(5)"));
        assert!(result.contains(".await?"));
    }

    #[test]
    fn test_render_builder_with_terminal_method() {
        let r = RustRenderer;

        let spec = BuilderSpec::new("Builder")
            .call_arg("option", Value::bool(true))
            .terminal_method("build");

        assert_eq!(
            spec.render_inline(&r),
            "Builder::new().option(true).build()"
        );
    }

    #[test]
    fn test_render_block() {
        let r = RustRenderer;

        let block = Block::new(Value::ident("pool")).binding(
            "options",
            Value::builder(
                BuilderSpec::new("SqliteConnectOptions")
                    .call_arg("create_if_missing", Value::bool(true)),
            ),
        );

        let result = block.render(&r);
        assert!(result.contains("let options = SqliteConnectOptions::new()"));
        assert!(result.contains("pool"));
        assert!(result.starts_with('{'));
        assert!(result.ends_with('}'));
    }

    #[test]
    fn test_render_nested_builder() {
        let r = RustRenderer;

        let inner = BuilderSpec::new("SqliteConnectOptions")
            .call_arg("create_if_missing", Value::bool(true));

        let outer = BuilderSpec::new("PoolOptions")
            .call_arg("connect_with", Value::builder(inner))
            .async_()
            .try_();

        let result = outer.render_inline(&r);
        assert!(result.contains("SqliteConnectOptions::new().create_if_missing(true)"));
    }

    #[test]
    fn test_constructor_variants() {
        let r = RustRenderer;

        assert_eq!(
            r.render_constructor(&Constructor::static_new("Foo::Bar")),
            "Foo::Bar::new()"
        );
        assert_eq!(
            r.render_constructor(&Constructor::static_method(
                "sqlx::sqlite::SqliteConnectOptions",
                "from_str",
                vec![Value::env_var("DATABASE_URL")]
            )),
            "sqlx::sqlite::SqliteConnectOptions::from_str(&std::env::var(\"DATABASE_URL\")?)"
        );
        assert_eq!(
            r.render_constructor(&Constructor::class_new("Foo")),
            "Foo::new()"
        );
        assert_eq!(
            r.render_constructor(&Constructor::factory("create_foo")),
            "create_foo()"
        );
    }

    #[test]
    fn test_apply_config() {
        let r = RustRenderer;

        let max: Option<u64> = Some(10);
        let min: Option<u64> = None;
        let timeout: Option<u64> = Some(30);

        let spec = BuilderSpec::new("PoolOptions").apply_config([
            ("max_connections", max.map(Value::uint)),
            ("min_connections", min.map(Value::uint)),
            ("acquire_timeout", timeout.map(Value::duration_secs)),
        ]);

        let result = spec.render_inline(&r);
        assert_eq!(
            result,
            "PoolOptions::new().max_connections(10).acquire_timeout(std::time::Duration::from_secs(30))"
        );
    }
}
