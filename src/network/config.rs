//! Network configuration types for proxy and TLS settings

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main network configuration from jarvy.toml [network] section
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkConfig {
    /// HTTP proxy URL (e.g., "http://proxy.corp.com:8080")
    pub http_proxy: Option<String>,

    /// HTTPS proxy URL (e.g., "http://proxy.corp.com:8080")
    pub https_proxy: Option<String>,

    /// SOCKS5 proxy URL (e.g., "socks5://proxy.corp.com:1080")
    pub socks_proxy: Option<String>,

    /// Hosts to bypass proxy (comma-separated string or array)
    pub no_proxy: Option<NoProxy>,

    /// Proxy authentication configuration
    pub auth: Option<ProxyAuth>,

    /// TLS/SSL configuration
    pub tls: Option<TlsConfig>,

    /// Per-tool proxy overrides
    #[serde(default)]
    pub overrides: HashMap<String, NetworkOverride>,
}

/// NoProxy can be either a comma-separated string or an array of hosts
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NoProxy {
    /// Comma-separated list: "localhost,127.0.0.1,.corp.com"
    String(String),
    /// Array of hosts: ["localhost", "127.0.0.1", ".corp.com"]
    Array(Vec<String>),
}

impl NoProxy {
    /// Convert to a vector of hosts
    pub fn to_hosts(&self) -> Vec<String> {
        match self {
            NoProxy::String(s) => s.split(',').map(|h| h.trim().to_string()).collect(),
            NoProxy::Array(arr) => arr.clone(),
        }
    }

    /// Convert to comma-separated string for environment variable
    pub fn to_env_string(&self) -> String {
        self.to_hosts().join(",")
    }
}

/// Proxy authentication credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyAuth {
    /// Username for proxy authentication
    pub username: String,

    /// Password source (plain, env, file, or prompt)
    pub password: PasswordSource,
}

/// Source for proxy password - supports multiple secure retrieval methods
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PasswordSource {
    /// Plain text password (with warning about security)
    Plain(String),

    /// Password from environment variable
    #[serde(rename_all = "snake_case")]
    Env(String),

    /// Password from file
    #[serde(rename_all = "snake_case")]
    File(String),

    /// Prompt user for password interactively
    Prompt,
}

/// Custom deserializer for PasswordSource to handle object format
impl PasswordSource {
    /// Resolve the password to its actual value
    pub fn resolve(&self) -> Result<String, String> {
        match self {
            PasswordSource::Plain(p) => {
                eprintln!(
                    "Warning: Using plain text proxy password. Consider using env or file source."
                );
                Ok(p.clone())
            }
            PasswordSource::Env(var) => {
                std::env::var(var).map_err(|_| format!("Environment variable {} not set", var))
            }
            PasswordSource::File(path) => std::fs::read_to_string(path)
                .map(|s| s.trim().to_string())
                .map_err(|e| format!("Failed to read password file {}: {}", path, e)),
            PasswordSource::Prompt => {
                Err("Interactive password prompt not available in this context".to_string())
            }
        }
    }
}

/// TLS/SSL configuration for custom CA certificates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Path to CA bundle file
    pub ca_bundle: Option<String>,

    /// Skip TLS verification (dangerous, use only for testing)
    #[serde(default)]
    pub insecure: bool,
}

/// Per-tool network configuration override
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkOverride {
    /// Override HTTP proxy for this tool
    pub http_proxy: Option<String>,

    /// Override HTTPS proxy for this tool
    pub https_proxy: Option<String>,

    /// Override SOCKS proxy for this tool
    pub socks_proxy: Option<String>,

    /// Override no_proxy for this tool
    pub no_proxy: Option<NoProxy>,

    /// Disable proxy entirely for this tool
    #[serde(default)]
    pub no_proxy_all: bool,
}

impl NetworkConfig {
    /// Check if any proxy is configured
    pub fn has_proxy(&self) -> bool {
        self.http_proxy.is_some() || self.https_proxy.is_some() || self.socks_proxy.is_some()
    }

    /// Get the effective proxy URL for HTTP requests
    pub fn effective_http_proxy(&self) -> Option<&String> {
        self.http_proxy.as_ref().or(self.https_proxy.as_ref())
    }

    /// Get the effective proxy URL for HTTPS requests
    pub fn effective_https_proxy(&self) -> Option<&String> {
        self.https_proxy.as_ref().or(self.http_proxy.as_ref())
    }

    /// Check if a host should bypass the proxy
    pub fn should_bypass(&self, host: &str) -> bool {
        if let Some(no_proxy) = &self.no_proxy {
            let hosts = no_proxy.to_hosts();
            for pattern in hosts {
                if pattern.starts_with('.') {
                    // Suffix match: .corp.com matches foo.corp.com
                    if host.ends_with(&pattern) || host == &pattern[1..] {
                        return true;
                    }
                } else if host == pattern {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_config_has_proxy() {
        let mut config = NetworkConfig::default();
        assert!(!config.has_proxy());

        config.https_proxy = Some("http://proxy:8080".to_string());
        assert!(config.has_proxy());
    }

    #[test]
    fn test_should_bypass_exact_match() {
        let mut config = NetworkConfig::default();
        config.no_proxy = Some(NoProxy::String("localhost,127.0.0.1".to_string()));

        assert!(config.should_bypass("localhost"));
        assert!(config.should_bypass("127.0.0.1"));
        assert!(!config.should_bypass("example.com"));
    }

    #[test]
    fn test_should_bypass_suffix_match() {
        let mut config = NetworkConfig::default();
        config.no_proxy = Some(NoProxy::String(".corp.com".to_string()));

        assert!(config.should_bypass("foo.corp.com"));
        assert!(config.should_bypass("bar.foo.corp.com"));
        assert!(config.should_bypass("corp.com"));
        assert!(!config.should_bypass("example.com"));
    }

    #[test]
    fn test_effective_proxy_fallback() {
        let mut config = NetworkConfig::default();
        config.https_proxy = Some("https://proxy:8080".to_string());

        // HTTP falls back to HTTPS proxy
        assert_eq!(
            config.effective_http_proxy(),
            Some(&"https://proxy:8080".to_string())
        );
        assert_eq!(
            config.effective_https_proxy(),
            Some(&"https://proxy:8080".to_string())
        );
    }
}
