//! Rust function builder.

use baobao_codegen::CodeBuilder;

/// A parameter in a Rust function.
#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: String,
}

impl Param {
    pub fn new(name: impl Into<String>, ty: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ty: ty.into(),
        }
    }
}

/// Builder for Rust functions.
#[derive(Debug, Clone)]
pub struct Fn {
    name: String,
    doc: Option<String>,
    attrs: Vec<String>,
    is_public: bool,
    is_async: bool,
    params: Vec<Param>,
    return_type: Option<String>,
    body: Vec<String>,
}

impl Fn {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            doc: None,
            attrs: Vec::new(),
            is_public: true,
            is_async: false,
            params: Vec::new(),
            return_type: None,
            body: Vec::new(),
        }
    }

    pub fn doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
    }

    pub fn attr(mut self, attr: impl Into<String>) -> Self {
        self.attrs.push(attr.into());
        self
    }

    pub fn private(mut self) -> Self {
        self.is_public = false;
        self
    }

    pub fn async_(mut self) -> Self {
        self.is_async = true;
        self
    }

    pub fn param(mut self, param: Param) -> Self {
        self.params.push(param);
        self
    }

    pub fn returns(mut self, ty: impl Into<String>) -> Self {
        self.return_type = Some(ty.into());
        self
    }

    /// Add a line to the function body.
    pub fn body_line(mut self, line: impl Into<String>) -> Self {
        self.body.push(line.into());
        self
    }

    /// Add raw body content (can contain multiple lines).
    pub fn body(mut self, content: impl Into<String>) -> Self {
        for line in content.into().lines() {
            self.body.push(line.to_string());
        }
        self
    }

    /// Render the function to a CodeBuilder.
    pub fn render(&self, builder: CodeBuilder) -> CodeBuilder {
        let builder = if let Some(doc) = &self.doc {
            builder.rust_doc(doc)
        } else {
            builder
        };

        let builder = self
            .attrs
            .iter()
            .fold(builder, |b, attr| b.line(&format!("#[{}]", attr)));

        let vis = if self.is_public { "pub " } else { "" };
        let async_kw = if self.is_async { "async " } else { "" };

        let params_str = self
            .params
            .iter()
            .map(|p| {
                if p.ty.is_empty() {
                    p.name.clone()
                } else {
                    format!("{}: {}", p.name, p.ty)
                }
            })
            .collect::<Vec<_>>()
            .join(", ");

        let signature = match &self.return_type {
            Some(ret) => format!(
                "{}{}fn {}({}) -> {} {{",
                vis, async_kw, self.name, params_str, ret
            ),
            None => format!("{}{}fn {}({}) {{", vis, async_kw, self.name, params_str),
        };

        let builder = builder.line(&signature).indent();

        let builder = self.body.iter().fold(builder, |b, line| b.line(line));

        builder.dedent().line("}")
    }

    /// Build the function as a string.
    pub fn build(&self) -> String {
        self.render(CodeBuilder::rust()).build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_fn() {
        let f = Fn::new("greet").build();
        assert!(f.contains("pub fn greet() {"));
    }

    #[test]
    fn test_fn_with_params() {
        let f = Fn::new("add")
            .param(Param::new("a", "i32"))
            .param(Param::new("b", "i32"))
            .returns("i32")
            .body_line("a + b")
            .build();
        assert!(f.contains("pub fn add(a: i32, b: i32) -> i32 {"));
        assert!(f.contains("a + b"));
    }

    #[test]
    fn test_async_fn() {
        let f = Fn::new("fetch").async_().returns("Result<String>").build();
        assert!(f.contains("pub async fn fetch() -> Result<String> {"));
    }

    #[test]
    fn test_private_fn() {
        let f = Fn::new("helper").private().build();
        assert!(f.contains("fn helper() {"));
        assert!(!f.contains("pub"));
    }

    #[test]
    fn test_fn_with_doc() {
        let f = Fn::new("run").doc("Execute the command").build();
        assert!(f.contains("/// Execute the command"));
    }
}
