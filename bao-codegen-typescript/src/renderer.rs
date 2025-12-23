//! TypeScript-specific renderer for language-agnostic expressions.
//!
//! This module implements the [`Renderer`] trait for TypeScript, translating
//! [`Value`], [`BuilderSpec`], and [`Block`] into valid TypeScript syntax.

use baobao_codegen::builder::{
    Binding, Block, BuilderSpec, Constructor, RenderOptions, Renderer, Terminal, Value,
};
use baobao_core::to_camel_case;

/// TypeScript language renderer.
///
/// Renders language-agnostic expressions to valid TypeScript syntax.
#[derive(Debug, Clone, Copy, Default)]
pub struct TypeScriptRenderer;

impl TypeScriptRenderer {
    /// Create a new TypeScript renderer.
    pub fn new() -> Self {
        Self
    }

    /// Render a binding to TypeScript syntax.
    fn render_binding(&self, binding: &Binding, opts: &RenderOptions) -> String {
        let indent = opts.indent_str();
        let keyword = if binding.mutable { "let" } else { "const" };
        let value = self.render_value(&binding.value, &opts.nested());
        format!("{}{} {} = {};", indent, keyword, binding.name, value)
    }
}

impl Renderer for TypeScriptRenderer {
    fn render_value(&self, value: &Value, opts: &RenderOptions) -> String {
        match value {
            Value::Bool(v) => v.to_string(),
            Value::Int(v) => v.to_string(),
            Value::UInt(v) => v.to_string(),
            // JavaScript floats don't need special handling
            Value::Float(v) => v.to_string(),
            Value::String(v) => format!("\"{}\"", v),
            Value::Ident(v) => v.clone(),
            Value::Duration { millis } => {
                // TypeScript uses milliseconds directly as numbers
                millis.to_string()
            }
            Value::EnumVariant { path, variant } => {
                // TypeScript enums: EnumName.Variant
                format!("{}.{}", path, variant)
            }
            Value::EnvVar { name, .. } => {
                // TypeScript/Bun: process.env.NAME or Bun.env.NAME
                format!("process.env.{}", name)
            }
            Value::Try(inner) => {
                // TypeScript doesn't have ? operator, just render the value
                // (error handling is done differently, often with try/catch)
                self.render_value(inner, opts)
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
            // Multi-line format
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

        // TypeScript uses IIFE for block expressions
        let mut result = String::from("(() => {\n");

        // Render bindings
        for binding in &block.bindings {
            result.push_str(&self.render_binding(binding, &inner_opts));
            result.push('\n');
        }

        // Render body with return
        result.push_str(&inner_indent);
        result.push_str("return ");
        result.push_str(&self.render_value(&block.body, &inner_opts));
        result.push_str(";\n");

        result.push_str(&indent);
        result.push_str("})()");

        result
    }

    fn transform_method_name(&self, name: &str) -> String {
        // TypeScript uses camelCase for method names
        to_camel_case(name)
    }

    fn render_constructor(&self, ctor: &Constructor) -> String {
        match ctor {
            Constructor::StaticNew { type_path } => {
                // TypeScript uses `new TypeName()`
                format!("new {}()", type_path)
            }
            Constructor::StaticMethod {
                type_path,
                method,
                args,
            } => {
                let opts = RenderOptions::inline();
                let rendered_args: Vec<String> =
                    args.iter().map(|a| self.render_value(a, &opts)).collect();
                let method_name = to_camel_case(method);
                format!(
                    "{}.{}({})",
                    type_path,
                    method_name,
                    rendered_args.join(", ")
                )
            }
            Constructor::ClassNew { type_name } => {
                format!("new {}()", type_name)
            }
            Constructor::Factory { name } => {
                format!("{}()", name)
            }
        }
    }

    fn render_terminal(&self, terminal: &Terminal) -> String {
        let mut result = String::new();

        if let Some(method) = &terminal.method {
            let method_name = to_camel_case(method);
            result.push_str(&format!(".{}()", method_name));
        }

        if terminal.is_async {
            // In TypeScript, await comes before the expression
            // But since we're appending, we handle this in the caller
            // For now, we'll note that this needs await
        }

        // TypeScript doesn't have ? operator, errors handled differently

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_values() {
        let r = TypeScriptRenderer;
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
        let r = TypeScriptRenderer;
        let opts = RenderOptions::inline();

        // TypeScript uses raw milliseconds
        assert_eq!(
            r.render_value(&Value::Duration { millis: 5000 }, &opts),
            "5000"
        );
        assert_eq!(
            r.render_value(&Value::Duration { millis: 100 }, &opts),
            "100"
        );
    }

    #[test]
    fn test_render_env_var() {
        let r = TypeScriptRenderer;
        let opts = RenderOptions::inline();

        let value = Value::EnvVar {
            name: "DATABASE_URL".into(),
            by_ref: false,
        };
        assert_eq!(r.render_value(&value, &opts), "process.env.DATABASE_URL");
    }

    #[test]
    fn test_render_enum_variant() {
        let r = TypeScriptRenderer;
        let opts = RenderOptions::inline();

        let value = Value::EnumVariant {
            path: "JournalMode".into(),
            variant: "Wal".into(),
        };
        assert_eq!(r.render_value(&value, &opts), "JournalMode.Wal");
    }

    #[test]
    fn test_render_builder_inline() {
        let r = TypeScriptRenderer;

        let spec = BuilderSpec::new("PoolOptions")
            .call_arg("max_connections", Value::uint(10))
            .call_arg("min_connections", Value::uint(5));

        assert_eq!(
            spec.render_inline(&r),
            "new PoolOptions().maxConnections(10).minConnections(5)"
        );
    }

    #[test]
    fn test_transform_method_name() {
        let r = TypeScriptRenderer;
        assert_eq!(r.transform_method_name("max_connections"), "maxConnections");
        assert_eq!(
            r.transform_method_name("create_if_missing"),
            "createIfMissing"
        );
    }

    #[test]
    fn test_constructor_variants() {
        let r = TypeScriptRenderer;

        assert_eq!(
            r.render_constructor(&Constructor::static_new("Options")),
            "new Options()"
        );
        assert_eq!(
            r.render_constructor(&Constructor::class_new("PoolOptions")),
            "new PoolOptions()"
        );
        assert_eq!(
            r.render_constructor(&Constructor::factory("createOptions")),
            "createOptions()"
        );
    }
}
