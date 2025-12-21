//! Rust function builder.

use baobao_codegen::builder::{CodeBuilder, CodeFragment, Renderable};

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

/// A match arm (pattern => body).
#[derive(Debug, Clone)]
pub struct Arm {
    pattern: String,
    body: Vec<String>,
}

impl Arm {
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            body: Vec::new(),
        }
    }

    /// Add a single-line body (rendered as `pattern => body,`).
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = vec![body.into()];
        self
    }

    /// Add a multi-line body (rendered as block `pattern => { ... }`).
    pub fn body_block(mut self, content: impl Into<String>) -> Self {
        for line in content.into().lines() {
            self.body.push(line.to_string());
        }
        self
    }
}

/// Builder for Rust match expressions.
#[derive(Debug, Clone)]
pub struct Match {
    expr: String,
    arms: Vec<Arm>,
}

impl Match {
    pub fn new(expr: impl Into<String>) -> Self {
        Self {
            expr: expr.into(),
            arms: Vec::new(),
        }
    }

    /// Add a match arm.
    pub fn arm(mut self, arm: Arm) -> Self {
        self.arms.push(arm);
        self
    }

    /// Render the match expression to a CodeBuilder.
    pub fn render(&self, builder: CodeBuilder) -> CodeBuilder {
        let builder = builder.line(&format!("match {} {{", self.expr)).indent();

        let builder = self.arms.iter().fold(builder, |b, arm| {
            if arm.body.is_empty() {
                b.line(&format!("{} => {{}},", arm.pattern))
            } else if arm.body.len() == 1 {
                b.line(&format!("{} => {},", arm.pattern, arm.body[0]))
            } else {
                let b = b.line(&format!("{} => {{", arm.pattern)).indent();
                let b = arm.body.iter().fold(b, |b, line| b.line(line));
                b.dedent().line("}")
            }
        });

        builder.dedent().line("}")
    }

    /// Build the match expression as a string.
    pub fn build(&self) -> String {
        self.render(CodeBuilder::rust()).build()
    }

    /// Convert arms to code fragments.
    fn arms_to_fragments(&self) -> Vec<CodeFragment> {
        self.arms
            .iter()
            .flat_map(|arm| {
                if arm.body.is_empty() {
                    vec![CodeFragment::Line(format!("{} => {{}},", arm.pattern))]
                } else if arm.body.len() == 1 {
                    vec![CodeFragment::Line(format!(
                        "{} => {},",
                        arm.pattern, arm.body[0]
                    ))]
                } else {
                    vec![CodeFragment::Block {
                        header: format!("{} => {{", arm.pattern),
                        body: arm
                            .body
                            .iter()
                            .map(|line| CodeFragment::Line(line.clone()))
                            .collect(),
                        close: Some("}".to_string()),
                    }]
                }
            })
            .collect()
    }
}

impl Renderable for Match {
    fn to_fragments(&self) -> Vec<CodeFragment> {
        vec![CodeFragment::Block {
            header: format!("match {} {{", self.expr),
            body: self.arms_to_fragments(),
            close: Some("}".to_string()),
        }]
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

    /// Add an attribute to the function.
    ///
    /// Used for proc-macro attributes like `#[tokio::main]`, `#[test]`, etc.
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

    /// Conditionally mark the function as async.
    pub fn async_if(self, condition: bool) -> Self {
        if condition { self.async_() } else { self }
    }

    /// Conditionally add an attribute.
    pub fn attr_if(self, condition: bool, attr: impl Into<String>) -> Self {
        if condition { self.attr(attr) } else { self }
    }

    /// Conditionally make the function private.
    pub fn private_if(self, condition: bool) -> Self {
        if condition { self.private() } else { self }
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

    /// Add a match expression to the body.
    pub fn body_match(mut self, match_expr: &Match) -> Self {
        for line in match_expr.build().lines() {
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

    /// Format the function signature.
    fn format_signature(&self) -> String {
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

        match &self.return_type {
            Some(ret) => format!(
                "{}{}fn {}({}) -> {} {{",
                vis, async_kw, self.name, params_str, ret
            ),
            None => format!("{}{}fn {}({}) {{", vis, async_kw, self.name, params_str),
        }
    }
}

impl Renderable for Fn {
    fn to_fragments(&self) -> Vec<CodeFragment> {
        let mut fragments = Vec::new();

        if let Some(doc) = &self.doc {
            fragments.push(CodeFragment::RustDoc(doc.clone()));
        }

        for attr in &self.attrs {
            fragments.push(CodeFragment::Line(format!("#[{}]", attr)));
        }

        let body: Vec<CodeFragment> = self
            .body
            .iter()
            .map(|line| CodeFragment::Line(line.clone()))
            .collect();

        fragments.push(CodeFragment::Block {
            header: self.format_signature(),
            body,
            close: Some("}".to_string()),
        });

        fragments
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

    #[test]
    fn test_match_simple() {
        let m = Match::new("cmd")
            .arm(Arm::new("Command::Start").body("start()"))
            .arm(Arm::new("Command::Stop").body("stop()"))
            .build();
        assert!(m.contains("match cmd {"));
        assert!(m.contains("Command::Start => start(),"));
        assert!(m.contains("Command::Stop => stop(),"));
    }

    #[test]
    fn test_match_with_block() {
        let m = Match::new("value")
            .arm(Arm::new("Some(x)").body_block("println!(\"{}\", x);\nx"))
            .arm(Arm::new("None").body("0"))
            .build();
        assert!(m.contains("Some(x) => {"));
        assert!(m.contains("println!"));
        assert!(m.contains("None => 0,"));
    }

    #[test]
    fn test_match_empty_arm() {
        let m = Match::new("opt")
            .arm(Arm::new("Some(_)"))
            .arm(Arm::new("None"))
            .build();
        assert!(m.contains("Some(_) => {},"));
        assert!(m.contains("None => {},"));
    }

    #[test]
    fn test_fn_with_match() {
        let match_expr = Match::new("cmd")
            .arm(Arm::new("Command::Run").body("run()"))
            .arm(Arm::new("_").body("Ok(())"));

        let f = Fn::new("handle")
            .param(Param::new("cmd", "Command"))
            .returns("Result<()>")
            .body_match(&match_expr)
            .build();

        assert!(f.contains("pub fn handle(cmd: Command) -> Result<()>"));
        assert!(f.contains("match cmd {"));
        assert!(f.contains("Command::Run => run(),"));
    }
}
