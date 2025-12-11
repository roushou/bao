//! TypeScript function builder.

use baobao_codegen::builder::{CodeBuilder, CodeFragment, Renderable};

/// A parameter in a TypeScript function.
#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: String,
    pub optional: bool,
}

impl Param {
    pub fn new(name: impl Into<String>, ty: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ty: ty.into(),
            optional: false,
        }
    }

    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }
}

/// Builder for TypeScript functions.
#[derive(Debug, Clone)]
pub struct Fn {
    name: String,
    doc: Option<String>,
    exported: bool,
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
            exported: true,
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

    pub fn private(mut self) -> Self {
        self.exported = false;
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
            builder.jsdoc(doc)
        } else {
            builder
        };

        let export = if self.exported { "export " } else { "" };
        let async_kw = if self.is_async { "async " } else { "" };

        let params_str = self
            .params
            .iter()
            .map(|p| {
                let optional = if p.optional { "?" } else { "" };
                format!("{}{}: {}", p.name, optional, p.ty)
            })
            .collect::<Vec<_>>()
            .join(", ");

        let signature = match &self.return_type {
            Some(ret) => format!(
                "{}{}function {}({}): {} {{",
                export, async_kw, self.name, params_str, ret
            ),
            None => format!(
                "{}{}function {}({}) {{",
                export, async_kw, self.name, params_str
            ),
        };

        let builder = builder.line(&signature).indent();
        let builder = self.body.iter().fold(builder, |b, line| b.line(line));
        builder.dedent().line("}")
    }

    /// Build the function as a string.
    pub fn build(&self) -> String {
        self.render(CodeBuilder::typescript()).build()
    }

    /// Format the function signature.
    fn format_signature(&self) -> String {
        let export = if self.exported { "export " } else { "" };
        let async_kw = if self.is_async { "async " } else { "" };

        let params_str = self
            .params
            .iter()
            .map(|p| {
                let optional = if p.optional { "?" } else { "" };
                format!("{}{}: {}", p.name, optional, p.ty)
            })
            .collect::<Vec<_>>()
            .join(", ");

        match &self.return_type {
            Some(ret) => format!(
                "{}{}function {}({}): {} {{",
                export, async_kw, self.name, params_str, ret
            ),
            None => format!(
                "{}{}function {}({}) {{",
                export, async_kw, self.name, params_str
            ),
        }
    }
}

impl Renderable for Fn {
    fn to_fragments(&self) -> Vec<CodeFragment> {
        let mut fragments = Vec::new();

        // Doc comment
        if let Some(doc) = &self.doc {
            fragments.push(CodeFragment::JsDoc(doc.clone()));
        }

        // Function body
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
        assert!(f.contains("export function greet() {"));
    }

    #[test]
    fn test_fn_with_params() {
        let f = Fn::new("add")
            .param(Param::new("a", "number"))
            .param(Param::new("b", "number"))
            .returns("number")
            .body_line("return a + b;")
            .build();
        assert!(f.contains("export function add(a: number, b: number): number {"));
        assert!(f.contains("return a + b;"));
    }

    #[test]
    fn test_async_fn() {
        let f = Fn::new("fetch").async_().returns("Promise<string>").build();
        assert!(f.contains("export async function fetch(): Promise<string> {"));
    }

    #[test]
    fn test_private_fn() {
        let f = Fn::new("helper").private().build();
        assert!(f.contains("function helper() {"));
        assert!(!f.contains("export"));
    }

    #[test]
    fn test_fn_with_doc() {
        let f = Fn::new("run").doc("Execute the command").build();
        assert!(f.contains("/** Execute the command */"));
    }

    #[test]
    fn test_fn_with_optional_param() {
        let f = Fn::new("greet")
            .param(Param::new("name", "string").optional())
            .build();
        assert!(f.contains("export function greet(name?: string) {"));
    }
}
