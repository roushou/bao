//! TypeScript method chain builder for fluent APIs.

/// A method call in a chain.
#[derive(Debug, Clone)]
struct Call {
    method: String,
    args: Vec<String>,
}

/// Builder for method chains (fluent API patterns).
#[derive(Debug, Clone)]
pub struct MethodChain {
    base: String,
    base_args: Vec<String>,
    calls: Vec<Call>,
}

impl MethodChain {
    /// Create a new method chain starting with a function call.
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            base: base.into(),
            base_args: Vec::new(),
            calls: Vec::new(),
        }
    }

    /// Add an argument to the base function call.
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.base_args.push(arg.into());
        self
    }

    /// Add a method call to the chain.
    pub fn call(mut self, method: impl Into<String>, arg: impl Into<String>) -> Self {
        self.calls.push(Call {
            method: method.into(),
            args: vec![arg.into()],
        });
        self
    }

    /// Add a method call with multiple arguments.
    pub fn call_args(mut self, method: impl Into<String>, args: Vec<String>) -> Self {
        self.calls.push(Call {
            method: method.into(),
            args,
        });
        self
    }

    /// Add a method call with no arguments.
    pub fn call_empty(mut self, method: impl Into<String>) -> Self {
        self.calls.push(Call {
            method: method.into(),
            args: Vec::new(),
        });
        self
    }

    /// Conditionally add a method call.
    pub fn call_if(
        self,
        condition: bool,
        method: impl Into<String>,
        arg: impl Into<String>,
    ) -> Self {
        if condition {
            self.call(method, arg)
        } else {
            self
        }
    }

    /// Conditionally add a method call using an Option.
    pub fn call_opt(self, method: impl Into<String>, arg: Option<impl Into<String>>) -> Self {
        match arg {
            Some(a) => self.call(method, a),
            None => self,
        }
    }

    /// Build the chain as a single-line string.
    pub fn build_inline(&self) -> String {
        let mut result = format!("{}({})", self.base, self.base_args.join(", "));

        for call in &self.calls {
            result.push_str(&format!(".{}({})", call.method, call.args.join(", ")));
        }

        result
    }

    /// Build the chain as a multi-line string with each call on its own line.
    pub fn build(&self) -> String {
        let mut result = format!("{}({})", self.base, self.base_args.join(", "));

        for call in &self.calls {
            result.push_str(&format!("\n  .{}({})", call.method, call.args.join(", ")));
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_chain() {
        let chain = MethodChain::new("cli")
            .arg("\"myapp\"")
            .call("version", "\"1.0.0\"")
            .build();
        assert_eq!(chain, "cli(\"myapp\")\n  .version(\"1.0.0\")");
    }

    #[test]
    fn test_chain_inline() {
        let chain = MethodChain::new("builder")
            .call("foo", "1")
            .call("bar", "2")
            .build_inline();
        assert_eq!(chain, "builder().foo(1).bar(2)");
    }

    #[test]
    fn test_chain_with_multiple_args() {
        let chain = MethodChain::new("create")
            .arg("\"name\"")
            .arg("42")
            .call("configure", "true")
            .build();
        assert_eq!(chain, "create(\"name\", 42)\n  .configure(true)");
    }

    #[test]
    fn test_call_if_true() {
        let chain = MethodChain::new("cli")
            .arg("\"app\"")
            .call_if(true, "description", "\"A CLI app\"")
            .build();
        assert_eq!(chain, "cli(\"app\")\n  .description(\"A CLI app\")");
    }

    #[test]
    fn test_call_if_false() {
        let chain = MethodChain::new("cli")
            .arg("\"app\"")
            .call_if(false, "description", "\"A CLI app\"")
            .build();
        assert_eq!(chain, "cli(\"app\")");
    }

    #[test]
    fn test_call_opt_some() {
        let desc = Some("\"A CLI app\"");
        let chain = MethodChain::new("cli")
            .arg("\"app\"")
            .call_opt("description", desc)
            .build();
        assert_eq!(chain, "cli(\"app\")\n  .description(\"A CLI app\")");
    }

    #[test]
    fn test_call_opt_none() {
        let desc: Option<&str> = None;
        let chain = MethodChain::new("cli")
            .arg("\"app\"")
            .call_opt("description", desc)
            .build();
        assert_eq!(chain, "cli(\"app\")");
    }

    #[test]
    fn test_call_empty() {
        let chain = MethodChain::new("builder")
            .call_empty("build")
            .build_inline();
        assert_eq!(chain, "builder().build()");
    }

    #[test]
    fn test_call_args() {
        let chain = MethodChain::new("obj")
            .call_args("method", vec!["a".into(), "b".into(), "c".into()])
            .build_inline();
        assert_eq!(chain, "obj().method(a, b, c)");
    }
}
