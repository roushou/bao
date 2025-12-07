use serde::Deserialize;

/// Configuration for HTTP client
#[derive(Debug, Deserialize, Clone, Default)]
pub struct HttpConfig {
    /// Request timeout in seconds
    pub timeout: Option<u64>,

    /// User agent string
    pub user_agent: Option<String>,
}

#[cfg(test)]
mod tests {
    use crate::Schema;

    fn parse(content: &str) -> Schema {
        toml::from_str(content).expect("Failed to parse TOML")
    }

    #[test]
    fn test_http_config() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [context.http]
            "#,
        );

        let http = schema.context.get("http").unwrap();
        assert!(matches!(http, super::super::ContextField::Http(_)));
        assert_eq!(http.env(), None);
        assert_eq!(http.rust_type(), "reqwest::Client");
        assert!(!http.is_async());
    }

    #[test]
    fn test_http_with_options() {
        let schema = parse(
            r#"
            [cli]
            name = "test"

            [context.http]
            timeout = 30
            user_agent = "my-cli/1.0"
            "#,
        );

        let http = schema.context.get("http").unwrap();
        if let super::super::ContextField::Http(config) = http {
            assert_eq!(config.timeout, Some(30));
            assert_eq!(config.user_agent, Some("my-cli/1.0".to_string()));
        } else {
            panic!("Expected Http variant");
        }
    }
}
