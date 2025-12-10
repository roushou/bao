//! TypeScript const declaration builder.

use baobao_codegen::CodeBuilder;

/// Builder for TypeScript const declarations.
#[derive(Debug, Clone)]
pub struct Const {
    name: String,
    value: String,
    ty: Option<String>,
    exported: bool,
}

impl Const {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            ty: None,
            exported: true,
        }
    }

    /// Add a type annotation.
    pub fn ty(mut self, ty: impl Into<String>) -> Self {
        self.ty = Some(ty.into());
        self
    }

    /// Make this const private (not exported).
    pub fn private(mut self) -> Self {
        self.exported = false;
        self
    }

    /// Render the const declaration to a CodeBuilder.
    pub fn render(&self, builder: CodeBuilder) -> CodeBuilder {
        let export = if self.exported { "export " } else { "" };

        let type_annotation = match &self.ty {
            Some(ty) => format!(": {}", ty),
            None => String::new(),
        };

        // Check if value is multiline
        if self.value.contains('\n') {
            let mut lines = self.value.lines();
            let first = lines.next().unwrap_or("");
            let builder = builder.line(&format!(
                "{}const {}{} = {}",
                export, self.name, type_annotation, first
            ));

            lines.fold(builder, |b, line| b.line(line))
        } else {
            builder.line(&format!(
                "{}const {}{} = {};",
                export, self.name, type_annotation, self.value
            ))
        }
    }

    /// Build the const declaration as a string.
    pub fn build(&self) -> String {
        self.render(CodeBuilder::typescript()).build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_const() {
        let c = Const::new("foo", "42").build();
        assert_eq!(c, "export const foo = 42;\n");
    }

    #[test]
    fn test_const_with_type() {
        let c = Const::new("name", "\"hello\"").ty("string").build();
        assert_eq!(c, "export const name: string = \"hello\";\n");
    }

    #[test]
    fn test_private_const() {
        let c = Const::new("secret", "123").private().build();
        assert_eq!(c, "const secret = 123;\n");
    }

    #[test]
    fn test_const_with_object() {
        let c = Const::new("config", "{ debug: true }").build();
        assert_eq!(c, "export const config = { debug: true };\n");
    }
}
