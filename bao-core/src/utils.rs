//! Shared utility functions for code generation.

/// Convert a string to PascalCase (e.g., "hello_world" -> "HelloWorld")
pub fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}

/// Convert a string to snake_case (e.g., "HelloWorld" -> "hello_world")
pub fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap());
    }
    result.replace('-', "_")
}

/// Convert a TOML value to its string representation
pub fn toml_value_to_string(value: &toml::Value) -> String {
    match value {
        toml::Value::String(s) => s.clone(),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("hello"), "Hello");
        assert_eq!(to_pascal_case("hello_world"), "HelloWorld");
        assert_eq!(to_pascal_case("foo_bar_baz"), "FooBarBaz");
        assert_eq!(to_pascal_case("hElLo"), "HElLo");
        assert_eq!(to_pascal_case(""), "");
    }

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("Hello"), "hello");
        assert_eq!(to_snake_case("HelloWorld"), "hello_world");
        assert_eq!(to_snake_case("FooBarBaz"), "foo_bar_baz");
        assert_eq!(to_snake_case("hello-world"), "hello_world");
        assert_eq!(to_snake_case(""), "");
    }

    #[test]
    fn test_toml_value_to_string() {
        assert_eq!(
            toml_value_to_string(&toml::Value::String("hello".to_string())),
            "hello"
        );
        assert_eq!(toml_value_to_string(&toml::Value::Integer(42)), "42");
        assert_eq!(
            toml_value_to_string(&toml::Value::Float(std::f64::consts::PI)),
            std::f64::consts::PI.to_string()
        );
        assert_eq!(toml_value_to_string(&toml::Value::Boolean(true)), "true");
    }
}
