//! Rust impl block builder.

use baobao_codegen::builder::{CodeBuilder, CodeFragment, Renderable};

use super::Fn;

/// Builder for Rust impl blocks.
#[derive(Debug, Clone)]
pub struct Impl {
    type_name: String,
    trait_name: Option<String>,
    methods: Vec<Fn>,
}

impl Impl {
    pub fn new(type_name: impl Into<String>) -> Self {
        Self {
            type_name: type_name.into(),
            trait_name: None,
            methods: Vec::new(),
        }
    }

    /// Create an impl block for a trait.
    pub fn for_trait(mut self, trait_name: impl Into<String>) -> Self {
        self.trait_name = Some(trait_name.into());
        self
    }

    pub fn method(mut self, method: Fn) -> Self {
        self.methods.push(method);
        self
    }

    /// Render the impl block to a CodeBuilder.
    pub fn render(&self, builder: CodeBuilder) -> CodeBuilder {
        let header = match &self.trait_name {
            Some(trait_name) => format!("impl {} for {} {{", trait_name, self.type_name),
            None => format!("impl {} {{", self.type_name),
        };

        let builder = builder.line(&header).indent();

        let builder = self
            .methods
            .iter()
            .enumerate()
            .fold(builder, |b, (i, method)| {
                let b = if i > 0 { b.blank() } else { b };
                method.render(b)
            });

        builder.dedent().line("}")
    }

    /// Build the impl block as a string.
    pub fn build(&self) -> String {
        self.render(CodeBuilder::rust()).build()
    }

    /// Format the impl header.
    fn format_header(&self) -> String {
        match &self.trait_name {
            Some(trait_name) => format!("impl {} for {} {{", trait_name, self.type_name),
            None => format!("impl {} {{", self.type_name),
        }
    }

    /// Convert methods to code fragments.
    fn methods_to_fragments(&self) -> Vec<CodeFragment> {
        self.methods
            .iter()
            .enumerate()
            .flat_map(|(i, method)| {
                let mut fragments = Vec::new();
                if i > 0 {
                    fragments.push(CodeFragment::Blank);
                }
                fragments.extend(method.to_fragments());
                fragments
            })
            .collect()
    }
}

impl Renderable for Impl {
    fn to_fragments(&self) -> Vec<CodeFragment> {
        vec![CodeFragment::Block {
            header: self.format_header(),
            body: self.methods_to_fragments(),
            close: Some("}".to_string()),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::fns::Param;

    #[test]
    fn test_empty_impl() {
        let i = Impl::new("Foo").build();
        assert!(i.contains("impl Foo {"));
    }

    #[test]
    fn test_impl_with_method() {
        let i = Impl::new("Counter")
            .method(
                Fn::new("increment")
                    .param(Param::new("&mut self", ""))
                    .body_line("self.count += 1;"),
            )
            .build();
        assert!(i.contains("impl Counter {"));
        assert!(i.contains("pub fn increment(&mut self) {"));
    }

    #[test]
    fn test_impl_for_trait() {
        let i = Impl::new("MyStruct")
            .for_trait("Display")
            .method(
                Fn::new("fmt")
                    .param(Param::new("&self", ""))
                    .param(Param::new("f", "&mut std::fmt::Formatter<'_>"))
                    .returns("std::fmt::Result"),
            )
            .build();
        assert!(i.contains("impl Display for MyStruct {"));
    }

    #[test]
    fn test_impl_with_multiple_methods() {
        let i = Impl::new("Foo")
            .method(Fn::new("bar"))
            .method(Fn::new("baz"))
            .build();
        assert!(i.contains("pub fn bar()"));
        assert!(i.contains("pub fn baz()"));
    }
}
