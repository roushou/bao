//! TypeScript/JavaScript object literal builder.

use baobao_codegen::builder::{CodeBuilder, CodeFragment, Renderable};

use super::arrays::JsArray;

/// A property in a JavaScript object literal.
#[derive(Debug, Clone)]
pub struct Property {
    pub key: String,
    pub value: PropertyValue,
}

/// The value of an object property.
#[derive(Debug, Clone)]
pub enum PropertyValue {
    /// A literal string value (will be quoted).
    String(String),
    /// A raw expression (will not be quoted).
    Raw(String),
    /// A nested object.
    Object(JsObject),
    /// An arrow function body.
    ArrowFn(ArrowFn),
    /// An array literal.
    Array(JsArray),
}

impl Property {
    /// Create a property with a string value (will be quoted).
    pub fn string(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: PropertyValue::String(value.into()),
        }
    }

    /// Create a property with a raw expression value (will not be quoted).
    pub fn raw(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: PropertyValue::Raw(value.into()),
        }
    }

    /// Create a property with a nested object value.
    pub fn object(key: impl Into<String>, value: JsObject) -> Self {
        Self {
            key: key.into(),
            value: PropertyValue::Object(value),
        }
    }

    /// Create a property with an arrow function value.
    pub fn arrow_fn(key: impl Into<String>, value: ArrowFn) -> Self {
        Self {
            key: key.into(),
            value: PropertyValue::ArrowFn(value),
        }
    }

    /// Create a property with an array value.
    pub fn array(key: impl Into<String>, value: JsArray) -> Self {
        Self {
            key: key.into(),
            value: PropertyValue::Array(value),
        }
    }

    /// Create a shorthand property where key equals the variable name.
    pub fn shorthand(name: impl Into<String>) -> Self {
        let n = name.into();
        Self {
            key: n.clone(),
            value: PropertyValue::Raw(n),
        }
    }
}

/// An arrow function for use as a property value.
#[derive(Debug, Clone)]
pub struct ArrowFn {
    pub params: String,
    pub is_async: bool,
    pub body: Vec<String>,
}

impl ArrowFn {
    pub fn new(params: impl Into<String>) -> Self {
        Self {
            params: params.into(),
            is_async: false,
            body: Vec::new(),
        }
    }

    pub fn async_(mut self) -> Self {
        self.is_async = true;
        self
    }

    pub fn body_line(mut self, line: impl Into<String>) -> Self {
        self.body.push(line.into());
        self
    }

    pub fn body_lines(mut self, lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        for line in lines {
            self.body.push(line.into());
        }
        self
    }
}

/// Builder for JavaScript/TypeScript object literals.
#[derive(Debug, Clone, Default)]
pub struct JsObject {
    properties: Vec<Property>,
}

impl JsObject {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a property with a string value (will be quoted).
    pub fn string(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.push(Property::string(key, value));
        self
    }

    /// Add a property with a raw expression value (will not be quoted).
    pub fn raw(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.push(Property::raw(key, value));
        self
    }

    /// Add a property with a nested object value.
    pub fn object(mut self, key: impl Into<String>, value: JsObject) -> Self {
        self.properties.push(Property::object(key, value));
        self
    }

    /// Add an arrow function property.
    pub fn arrow_fn(mut self, key: impl Into<String>, value: ArrowFn) -> Self {
        self.properties.push(Property::arrow_fn(key, value));
        self
    }

    /// Add an array property.
    pub fn array(mut self, key: impl Into<String>, value: JsArray) -> Self {
        self.properties.push(Property::array(key, value));
        self
    }

    /// Conditionally add an array property.
    pub fn array_if(self, condition: bool, key: impl Into<String>, value: JsArray) -> Self {
        if condition {
            self.array(key, value)
        } else {
            self
        }
    }

    /// Conditionally add an array property using an Option.
    pub fn array_opt(self, key: impl Into<String>, value: Option<JsArray>) -> Self {
        match value {
            Some(v) => self.array(key, v),
            None => self,
        }
    }

    /// Add a property with a TOML value (converted to TypeScript literal).
    ///
    /// - Strings are quoted
    /// - Numbers and booleans are raw
    /// - Other types are ignored
    pub fn toml(self, key: impl Into<String>, value: &toml::Value) -> Self {
        match value {
            toml::Value::String(s) => self.string(key, s),
            toml::Value::Integer(i) => self.raw(key, i.to_string()),
            toml::Value::Float(f) => self.raw(key, f.to_string()),
            toml::Value::Boolean(b) => self.raw(key, b.to_string()),
            _ => self,
        }
    }

    /// Conditionally add a TOML value property.
    pub fn toml_opt(self, key: impl Into<String>, value: Option<&toml::Value>) -> Self {
        match value {
            Some(v) => self.toml(key, v),
            None => self,
        }
    }

    /// Add an IR DefaultValue property, quoting strings appropriately.
    pub fn default_value(self, key: impl Into<String>, value: &baobao_ir::DefaultValue) -> Self {
        match value {
            baobao_ir::DefaultValue::String(s) => self.string(key, s),
            baobao_ir::DefaultValue::Int(i) => self.raw(key, i.to_string()),
            baobao_ir::DefaultValue::Float(f) => self.raw(key, f.to_string()),
            baobao_ir::DefaultValue::Bool(b) => self.raw(key, b.to_string()),
        }
    }

    /// Conditionally add an IR DefaultValue property.
    pub fn default_value_opt(
        self,
        key: impl Into<String>,
        value: Option<&baobao_ir::DefaultValue>,
    ) -> Self {
        match value {
            Some(v) => self.default_value(key, v),
            None => self,
        }
    }

    /// Add a shorthand property where key equals the variable name.
    pub fn shorthand(mut self, name: impl Into<String>) -> Self {
        self.properties.push(Property::shorthand(name));
        self
    }

    /// Conditionally add a string property.
    pub fn string_if(
        self,
        condition: bool,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        if condition {
            self.string(key, value)
        } else {
            self
        }
    }

    /// Conditionally add a string property using an Option.
    pub fn string_opt(self, key: impl Into<String>, value: Option<impl Into<String>>) -> Self {
        match value {
            Some(v) => self.string(key, v),
            None => self,
        }
    }

    /// Conditionally add a raw property.
    pub fn raw_if(self, condition: bool, key: impl Into<String>, value: impl Into<String>) -> Self {
        if condition {
            self.raw(key, value)
        } else {
            self
        }
    }

    /// Conditionally add a raw property using an Option.
    pub fn raw_opt(self, key: impl Into<String>, value: Option<impl Into<String>>) -> Self {
        match value {
            Some(v) => self.raw(key, v),
            None => self,
        }
    }

    /// Conditionally add a nested object property.
    pub fn object_if(self, condition: bool, key: impl Into<String>, value: JsObject) -> Self {
        if condition {
            self.object(key, value)
        } else {
            self
        }
    }

    /// Check if the object is empty.
    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
    }

    /// Render the object literal to a CodeBuilder.
    pub fn render(&self, builder: CodeBuilder) -> CodeBuilder {
        if self.properties.is_empty() {
            return builder.raw("{}");
        }

        let builder = builder.line("{").indent();
        let builder = self.render_properties(builder);
        builder.dedent().raw("}")
    }

    fn render_properties(&self, builder: CodeBuilder) -> CodeBuilder {
        self.properties
            .iter()
            .fold(builder, |b, prop| match &prop.value {
                PropertyValue::String(s) => b.line(&format!("{}: \"{}\",", prop.key, s)),
                PropertyValue::Raw(s) => b.line(&format!("{}: {},", prop.key, s)),
                PropertyValue::Object(obj) => {
                    let b = b.line(&format!("{}: {{", prop.key)).indent();
                    let b = obj.render_properties(b);
                    b.dedent().line("},")
                }
                PropertyValue::ArrowFn(func) => {
                    let async_kw = if func.is_async { "async " } else { "" };
                    let b = b.line(&format!(
                        "{}: {}({}) => {{",
                        prop.key, async_kw, func.params
                    ));
                    let b = b.indent();
                    let b = func.body.iter().fold(b, |b, line| b.line(line));
                    b.dedent().line("},")
                }
                PropertyValue::Array(arr) => b.line(&format!("{}: {},", prop.key, arr.build())),
            })
    }

    /// Build the object as a string.
    pub fn build(&self) -> String {
        self.render(CodeBuilder::typescript()).build()
    }

    /// Convert properties to code fragments.
    fn properties_to_fragments(&self) -> Vec<CodeFragment> {
        self.properties
            .iter()
            .map(|prop| match &prop.value {
                PropertyValue::String(s) => CodeFragment::Line(format!("{}: \"{}\",", prop.key, s)),
                PropertyValue::Raw(s) => CodeFragment::Line(format!("{}: {},", prop.key, s)),
                PropertyValue::Object(obj) => {
                    let body = obj.properties_to_fragments();
                    // Remove trailing comma from nested object for cleaner output
                    CodeFragment::Block {
                        header: format!("{}: {{", prop.key),
                        body,
                        close: Some("},".to_string()),
                    }
                }
                PropertyValue::ArrowFn(func) => {
                    let async_kw = if func.is_async { "async " } else { "" };
                    let body: Vec<CodeFragment> = func
                        .body
                        .iter()
                        .map(|line| CodeFragment::Line(line.clone()))
                        .collect();
                    CodeFragment::Block {
                        header: format!("{}: {}({}) => {{", prop.key, async_kw, func.params),
                        body,
                        close: Some("},".to_string()),
                    }
                }
                PropertyValue::Array(arr) => {
                    CodeFragment::Line(format!("{}: {},", prop.key, arr.build()))
                }
            })
            .collect()
    }
}

impl Renderable for JsObject {
    fn to_fragments(&self) -> Vec<CodeFragment> {
        if self.properties.is_empty() {
            return vec![CodeFragment::Raw("{}".to_string())];
        }

        vec![CodeFragment::Block {
            header: "{".to_string(),
            body: self.properties_to_fragments(),
            close: Some("}".to_string()),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_object() {
        let obj = JsObject::new().build();
        assert_eq!(obj, "{}");
    }

    #[test]
    fn test_object_with_string() {
        let obj = JsObject::new().string("name", "myapp").build();
        assert!(obj.contains("name: \"myapp\","));
    }

    #[test]
    fn test_object_with_raw() {
        let obj = JsObject::new().raw("count", "42").build();
        assert!(obj.contains("count: 42,"));
    }

    #[test]
    fn test_object_with_shorthand() {
        let obj = JsObject::new().shorthand("helloCommand").build();
        assert!(obj.contains("helloCommand: helloCommand,"));
    }

    #[test]
    fn test_nested_object() {
        let inner = JsObject::new().raw("foo", "fooCommand");
        let outer = JsObject::new()
            .string("name", "test")
            .object("commands", inner);
        let result = outer.build();
        assert!(result.contains("name: \"test\","));
        assert!(result.contains("commands:"));
        assert!(result.contains("foo: fooCommand,"));
    }

    #[test]
    fn test_conditional_string() {
        let obj = JsObject::new()
            .string("name", "test")
            .string_opt("desc", Some("A description"))
            .string_opt("missing", None::<&str>)
            .build();
        assert!(obj.contains("name: \"test\","));
        assert!(obj.contains("desc: \"A description\","));
        assert!(!obj.contains("missing"));
    }

    #[test]
    fn test_arrow_fn() {
        let func = ArrowFn::new("{ args, options }")
            .async_()
            .body_line("console.log(args);")
            .body_line("await run(args);");
        let obj = JsObject::new().arrow_fn("action", func).build();
        assert!(obj.contains("action: async ({ args, options }) => {"));
        assert!(obj.contains("console.log(args);"));
        assert!(obj.contains("await run(args);"));
        assert!(obj.contains("},"));
    }
}
