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
    use crate::Manifest;

    fn parse(content: &str) -> Manifest {
        toml::from_str(content).expect("Failed to parse TOML")
    }

    #[test]
    fn test_http_config() {
        let schema = parse(
            r#"
            [cli]
            name = "test"
            language = "rust"

            [context.http]
            "#,
        );

        let http = schema.context.http_config().unwrap();
        assert_eq!(http.timeout, None);
        assert_eq!(http.user_agent, None);
    }

    #[test]
    fn test_http_with_options() {
        let schema = parse(
            r#"
            [cli]
            name = "test"
            language = "rust"

            [context.http]
            timeout = 30
            user_agent = "my-cli/1.0"
            "#,
        );

        let http = schema.context.http_config().unwrap();
        assert_eq!(http.timeout, Some(30));
        assert_eq!(http.user_agent, Some("my-cli/1.0".to_string()));
    }
}
