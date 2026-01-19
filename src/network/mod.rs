//! Network configuration module for proxy and TLS settings
//!
//! This module provides comprehensive support for corporate network environments,
//! including HTTP/HTTPS/SOCKS proxies, custom CA certificates, and authentication.
//!
//! # Priority Order
//! 1. Environment variables (HTTP_PROXY, HTTPS_PROXY, etc.)
//! 2. Tool-specific overrides in [network.overrides.<tool>]
//! 3. Global config in [network] section
//!
//! # Example Configuration
//! ```toml
//! [network]
//! https_proxy = "http://proxy.corp.com:8080"
//! no_proxy = ["localhost", "127.0.0.1", ".corp.com"]
//!
//! [network.auth]
//! username = "jdoe"
//! password = { env = "PROXY_PASSWORD" }
//!
//! [network.tls]
//! ca_bundle = "/etc/ssl/certs/corporate-ca.crt"
//!
//! [network.overrides.git]
//! https_proxy = "http://git-proxy.corp.com:8888"
//! ```

pub mod auth;
pub mod config;
pub mod package_managers;
pub mod propagate;
pub mod resolve;
pub mod testing;

pub use auth::*;
pub use config::*;
pub use package_managers::*;
pub use propagate::*;
pub use resolve::*;
pub use testing::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_config_default() {
        let config = NetworkConfig::default();
        assert!(config.http_proxy.is_none());
        assert!(config.https_proxy.is_none());
        assert!(config.socks_proxy.is_none());
        assert!(config.no_proxy.is_none());
    }

    #[test]
    fn test_proxy_url_redaction() {
        let url = "http://user:secret@proxy.corp.com:8080";
        let redacted = redact_credentials(url);
        assert!(redacted.contains("user:***"));
        assert!(!redacted.contains("secret"));
    }

    #[test]
    fn test_no_proxy_parsing() {
        let no_proxy = NoProxy::String("localhost,127.0.0.1,.corp.com".to_string());
        let hosts = no_proxy.to_hosts();
        assert_eq!(hosts.len(), 3);
        assert!(hosts.contains(&"localhost".to_string()));
        assert!(hosts.contains(&"127.0.0.1".to_string()));
        assert!(hosts.contains(&".corp.com".to_string()));
    }

    #[test]
    fn test_no_proxy_array() {
        let no_proxy = NoProxy::Array(vec!["localhost".to_string(), "127.0.0.1".to_string()]);
        let hosts = no_proxy.to_hosts();
        assert_eq!(hosts.len(), 2);
    }

    #[test]
    fn test_password_source_variants() {
        let plain = PasswordSource::Plain("secret".to_string());
        assert!(matches!(plain, PasswordSource::Plain(_)));

        let env = PasswordSource::Env("PROXY_PASSWORD".to_string());
        assert!(matches!(env, PasswordSource::Env(_)));

        let file = PasswordSource::File("/path/to/password".to_string());
        assert!(matches!(file, PasswordSource::File(_)));

        let prompt = PasswordSource::Prompt;
        assert!(matches!(prompt, PasswordSource::Prompt));
    }
}

/// Redact credentials from a proxy URL for safe logging
pub fn redact_credentials(url: &str) -> String {
    // Pattern: http://user:password@host:port
    if let Some(at_pos) = url.find('@') {
        if let Some(proto_end) = url.find("://") {
            let before_creds = &url[..proto_end + 3];
            let after_at = &url[at_pos..];

            // Find username:password part
            let creds_part = &url[proto_end + 3..at_pos];
            if let Some(colon) = creds_part.find(':') {
                let username = &creds_part[..colon];
                return format!("{}{}:***{}", before_creds, username, after_at);
            }
        }
    }
    url.to_string()
}
