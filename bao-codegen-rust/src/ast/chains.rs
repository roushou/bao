//! Builder for Rust method chains (fluent API patterns).

/// A method call in a chain.
#[derive(Debug, Clone)]
struct MethodCall {
    name: String,
    args: Vec<String>,
}

/// Builder for fluent method chains like `Foo::new().bar(x).baz().await?`.
#[derive(Debug, Clone)]
pub struct MethodChain {
    base: String,
    calls: Vec<MethodCall>,
    is_await: bool,
    is_try: bool,
    indent: usize,
}

impl MethodChain {
    /// Create a new method chain starting from a base expression.
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            base: base.into(),
            calls: Vec::new(),
            is_await: false,
            is_try: false,
            indent: 16,
        }
    }

    /// Set the indentation for continuation lines (default: 16 spaces).
    pub fn indent(mut self, spaces: usize) -> Self {
        self.indent = spaces;
        self
    }

    /// Add a method call with no arguments.
    pub fn method(mut self, name: impl Into<String>) -> Self {
        self.calls.push(MethodCall {
            name: name.into(),
            args: Vec::new(),
        });
        self
    }

    /// Add a method call with a single argument.
    pub fn method_arg(mut self, name: impl Into<String>, arg: impl Into<String>) -> Self {
        self.calls.push(MethodCall {
            name: name.into(),
            args: vec![arg.into()],
        });
        self
    }

    /// Add a method call with multiple arguments.
    pub fn method_args(
        mut self,
        name: impl Into<String>,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.calls.push(MethodCall {
            name: name.into(),
            args: args.into_iter().map(|a| a.into()).collect(),
        });
        self
    }

    /// Conditionally add a method call if the condition is true.
    pub fn method_if(self, condition: bool, name: impl Into<String>) -> Self {
        if condition { self.method(name) } else { self }
    }

    /// Conditionally add a method call with argument if the condition is true.
    pub fn method_arg_if(
        self,
        condition: bool,
        name: impl Into<String>,
        arg: impl Into<String>,
    ) -> Self {
        if condition {
            self.method_arg(name, arg)
        } else {
            self
        }
    }

    /// Conditionally add a method call if the option is Some.
    pub fn method_arg_opt<T: ToString>(self, name: impl Into<String>, value: Option<T>) -> Self {
        match value {
            Some(v) => self.method_arg(name, v.to_string()),
            None => self,
        }
    }

    /// Add `.await` to the chain.
    pub fn await_(mut self) -> Self {
        self.is_await = true;
        self
    }

    /// Add `?` to the chain.
    pub fn try_(mut self) -> Self {
        self.is_try = true;
        self
    }

    /// Check if the chain has any method calls.
    pub fn has_calls(&self) -> bool {
        !self.calls.is_empty()
    }

    /// Build the method chain as a single-line string.
    pub fn build_inline(&self) -> String {
        let mut result = self.base.clone();

        for call in &self.calls {
            if call.args.is_empty() {
                result.push_str(&format!(".{}()", call.name));
            } else {
                result.push_str(&format!(".{}({})", call.name, call.args.join(", ")));
            }
        }

        if self.is_await {
            result.push_str(".await");
        }
        if self.is_try {
            result.push('?');
        }

        result
    }

    /// Build the method chain with each call on a new line.
    pub fn build(&self) -> String {
        if self.calls.is_empty() {
            let mut result = self.base.clone();
            if self.is_await {
                result.push_str(".await");
            }
            if self.is_try {
                result.push('?');
            }
            return result;
        }

        let indent = " ".repeat(self.indent);
        let mut result = self.base.clone();

        for call in &self.calls {
            if call.args.is_empty() {
                result.push_str(&format!("\n{}.{}()", indent, call.name));
            } else {
                result.push_str(&format!(
                    "\n{}.{}({})",
                    indent,
                    call.name,
                    call.args.join(", ")
                ));
            }
        }

        if self.is_await {
            result.push_str(".await");
        }
        if self.is_try {
            result.push('?');
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_chain() {
        let chain = MethodChain::new("Foo::new()")
            .method("bar")
            .method_arg("baz", "42")
            .build_inline();
        assert_eq!(chain, "Foo::new().bar().baz(42)");
    }

    #[test]
    fn test_chain_with_await_try() {
        let chain = MethodChain::new("client.get(url)")
            .method("send")
            .await_()
            .try_()
            .build_inline();
        assert_eq!(chain, "client.get(url).send().await?");
    }

    #[test]
    fn test_multiline_chain() {
        let chain = MethodChain::new("PoolOptions::new()")
            .indent(4)
            .method_arg("max_connections", "10")
            .method_arg("min_connections", "5")
            .build();
        assert!(chain.contains("PoolOptions::new()"));
        assert!(chain.contains("\n    .max_connections(10)"));
        assert!(chain.contains("\n    .min_connections(5)"));
    }

    #[test]
    fn test_conditional_methods() {
        let chain = MethodChain::new("Builder::new()")
            .method_if(true, "enabled")
            .method_if(false, "disabled")
            .method_arg_opt("value", Some(42))
            .method_arg_opt::<i32>("missing", None)
            .build_inline();
        assert_eq!(chain, "Builder::new().enabled().value(42)");
    }

    #[test]
    fn test_empty_chain() {
        let chain = MethodChain::new("value").await_().try_().build();
        assert_eq!(chain, "value.await?");
    }

    #[test]
    fn test_multiple_args() {
        let chain = MethodChain::new("map")
            .method_args("insert", ["key", "value"])
            .build_inline();
        assert_eq!(chain, "map.insert(key, value)");
    }
}
