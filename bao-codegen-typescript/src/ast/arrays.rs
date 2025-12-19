//! TypeScript/JavaScript array literal builder.

/// An element in a JavaScript array literal.
#[derive(Debug, Clone)]
pub enum ArrayElement {
    /// A literal string value (will be quoted).
    String(String),
    /// A raw expression (will not be quoted).
    Raw(String),
}

/// Builder for JavaScript/TypeScript array literals.
///
/// Supports the `as const` TypeScript assertion for literal types.
#[derive(Debug, Clone, Default)]
pub struct JsArray {
    elements: Vec<ArrayElement>,
    as_const: bool,
}

impl JsArray {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an array from string values (will be quoted).
    pub fn from_strings<I, S>(iter: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            elements: iter
                .into_iter()
                .map(|s| ArrayElement::String(s.into()))
                .collect(),
            as_const: false,
        }
    }

    /// Add a string element (will be quoted).
    pub fn string(mut self, value: impl Into<String>) -> Self {
        self.elements.push(ArrayElement::String(value.into()));
        self
    }

    /// Add a raw expression element (will not be quoted).
    pub fn raw(mut self, value: impl Into<String>) -> Self {
        self.elements.push(ArrayElement::Raw(value.into()));
        self
    }

    /// Add the `as const` TypeScript assertion.
    pub fn as_const(mut self) -> Self {
        self.as_const = true;
        self
    }

    /// Check if the array is empty.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Build the array literal as a string.
    pub fn build(&self) -> String {
        let elements_str = self
            .elements
            .iter()
            .map(|e| match e {
                ArrayElement::String(s) => format!("\"{}\"", s),
                ArrayElement::Raw(s) => s.clone(),
            })
            .collect::<Vec<_>>()
            .join(", ");

        if self.as_const {
            format!("[{}] as const", elements_str)
        } else {
            format!("[{}]", elements_str)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_array() {
        let arr = JsArray::new().build();
        assert_eq!(arr, "[]");
    }

    #[test]
    fn test_string_array() {
        let arr = JsArray::new().string("foo").string("bar").build();
        assert_eq!(arr, "[\"foo\", \"bar\"]");
    }

    #[test]
    fn test_raw_array() {
        let arr = JsArray::new().raw("42").raw("true").build();
        assert_eq!(arr, "[42, true]");
    }

    #[test]
    fn test_as_const() {
        let arr = JsArray::new().string("a").string("b").as_const().build();
        assert_eq!(arr, "[\"a\", \"b\"] as const");
    }

    #[test]
    fn test_from_strings() {
        let choices = vec!["dev", "prod", "test"];
        let arr = JsArray::from_strings(choices).as_const().build();
        assert_eq!(arr, "[\"dev\", \"prod\", \"test\"] as const");
    }

    #[test]
    fn test_mixed_array() {
        let arr = JsArray::new()
            .string("name")
            .raw("123")
            .string("value")
            .build();
        assert_eq!(arr, "[\"name\", 123, \"value\"]");
    }
}
