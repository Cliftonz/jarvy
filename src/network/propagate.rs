//! Proxy propagation to child processes and environment variables
//!
//! This module handles applying proxy settings to spawned commands and
//! generating environment variables for different tools.

use super::config::{NetworkConfig, TlsConfig};
use super::resolve::ResolvedProxy;
use std::collections::HashMap;
use std::process::Command;

/// Environment variable keys for proxy settings
pub mod env_keys {
    // Standard proxy variables (both cases for compatibility)
    pub const HTTP_PROXY_UPPER: &str = "HTTP_PROXY";
    pub const HTTP_PROXY_LOWER: &str = "http_proxy";
    pub const HTTPS_PROXY_UPPER: &str = "HTTPS_PROXY";
    pub const HTTPS_PROXY_LOWER: &str = "https_proxy";
    pub const NO_PROXY_UPPER: &str = "NO_PROXY";
    pub const NO_PROXY_LOWER: &str = "no_proxy";
    pub const ALL_PROXY_UPPER: &str = "ALL_PROXY";
    pub const ALL_PROXY_LOWER: &str = "all_proxy";

    // CA bundle variables for different tools
    pub const CURL_CA_BUNDLE: &str = "CURL_CA_BUNDLE";
    pub const SSL_CERT_FILE: &str = "SSL_CERT_FILE";
    pub const SSL_CERT_DIR: &str = "SSL_CERT_DIR";
    pub const REQUESTS_CA_BUNDLE: &str = "REQUESTS_CA_BUNDLE";
    pub const NODE_EXTRA_CA_CERTS: &str = "NODE_EXTRA_CA_CERTS";
    pub const GIT_SSL_CAINFO: &str = "GIT_SSL_CAINFO";
    pub const AWS_CA_BUNDLE: &str = "AWS_CA_BUNDLE";
}

/// Generate all environment variables for proxy and TLS settings
pub fn generate_env_vars(
    proxy: &ResolvedProxy,
    tls: Option<&TlsConfig>,
) -> HashMap<String, String> {
    let mut vars = proxy.to_env_vars();

    // Add CA bundle variables if configured
    if let Some(tls_config) = tls {
        if let Some(ref ca_bundle) = tls_config.ca_bundle {
            vars.insert(env_keys::CURL_CA_BUNDLE.to_string(), ca_bundle.clone());
            vars.insert(env_keys::SSL_CERT_FILE.to_string(), ca_bundle.clone());
            vars.insert(env_keys::REQUESTS_CA_BUNDLE.to_string(), ca_bundle.clone());
            vars.insert(env_keys::NODE_EXTRA_CA_CERTS.to_string(), ca_bundle.clone());
            vars.insert(env_keys::GIT_SSL_CAINFO.to_string(), ca_bundle.clone());
            vars.insert(env_keys::AWS_CA_BUNDLE.to_string(), ca_bundle.clone());
        }
    }

    vars
}

/// Apply proxy environment variables to a Command
pub fn apply_to_command(cmd: &mut Command, proxy: &ResolvedProxy, tls: Option<&TlsConfig>) {
    let vars = generate_env_vars(proxy, tls);
    for (key, value) in vars {
        cmd.env(key, value);
    }
}

/// Apply network config to a Command for a specific tool
pub fn apply_network_config(cmd: &mut Command, config: &NetworkConfig, tool_name: &str) {
    use super::resolve::ProxyResolver;

    let resolver = ProxyResolver::new(Some(config));
    let resolved = resolver.resolve_for_tool(tool_name);

    apply_to_command(cmd, &resolved, config.tls.as_ref());
}

/// Generate a shell script snippet that exports proxy variables
pub fn generate_shell_exports(proxy: &ResolvedProxy, tls: Option<&TlsConfig>) -> String {
    let vars = generate_env_vars(proxy, tls);
    let mut lines = Vec::new();

    for (key, value) in vars {
        // Escape single quotes in value
        let escaped = value.replace('\'', "'\\''");
        lines.push(format!("export {}='{}'", key, escaped));
    }

    lines.sort(); // Deterministic output
    lines.join("\n")
}

/// Clear proxy environment variables from a Command
pub fn clear_proxy_env(cmd: &mut Command) {
    cmd.env_remove(env_keys::HTTP_PROXY_UPPER);
    cmd.env_remove(env_keys::HTTP_PROXY_LOWER);
    cmd.env_remove(env_keys::HTTPS_PROXY_UPPER);
    cmd.env_remove(env_keys::HTTPS_PROXY_LOWER);
    cmd.env_remove(env_keys::NO_PROXY_UPPER);
    cmd.env_remove(env_keys::NO_PROXY_LOWER);
    cmd.env_remove(env_keys::ALL_PROXY_UPPER);
    cmd.env_remove(env_keys::ALL_PROXY_LOWER);
}

#[cfg(test)]
mod tests {
    use super::super::resolve::ProxySource;
    use super::*;

    #[test]
    fn test_generate_env_vars_basic() {
        let proxy = ResolvedProxy {
            http_proxy: Some("http://proxy:8080".to_string()),
            https_proxy: Some("https://proxy:8443".to_string()),
            socks_proxy: None,
            no_proxy: Some("localhost".to_string()),
            source: ProxySource::GlobalConfig,
        };

        let vars = generate_env_vars(&proxy, None);

        assert_eq!(
            vars.get("HTTP_PROXY"),
            Some(&"http://proxy:8080".to_string())
        );
        assert_eq!(
            vars.get("http_proxy"),
            Some(&"http://proxy:8080".to_string())
        );
        assert_eq!(
            vars.get("HTTPS_PROXY"),
            Some(&"https://proxy:8443".to_string())
        );
        assert_eq!(vars.get("NO_PROXY"), Some(&"localhost".to_string()));
    }

    #[test]
    fn test_generate_env_vars_with_ca() {
        let proxy = ResolvedProxy::default();
        let tls = TlsConfig {
            ca_bundle: Some("/etc/ssl/certs/ca.crt".to_string()),
            insecure: false,
        };

        let vars = generate_env_vars(&proxy, Some(&tls));

        assert_eq!(
            vars.get("CURL_CA_BUNDLE"),
            Some(&"/etc/ssl/certs/ca.crt".to_string())
        );
        assert_eq!(
            vars.get("SSL_CERT_FILE"),
            Some(&"/etc/ssl/certs/ca.crt".to_string())
        );
        assert_eq!(
            vars.get("NODE_EXTRA_CA_CERTS"),
            Some(&"/etc/ssl/certs/ca.crt".to_string())
        );
        assert_eq!(
            vars.get("GIT_SSL_CAINFO"),
            Some(&"/etc/ssl/certs/ca.crt".to_string())
        );
    }

    #[test]
    fn test_generate_shell_exports() {
        let proxy = ResolvedProxy {
            http_proxy: Some("http://proxy:8080".to_string()),
            https_proxy: None,
            socks_proxy: None,
            no_proxy: None,
            source: ProxySource::GlobalConfig,
        };

        let exports = generate_shell_exports(&proxy, None);

        assert!(exports.contains("export HTTP_PROXY='http://proxy:8080'"));
        assert!(exports.contains("export http_proxy='http://proxy:8080'"));
    }

    #[test]
    fn test_generate_shell_exports_escaping() {
        let proxy = ResolvedProxy {
            http_proxy: Some("http://user:p'ass@proxy:8080".to_string()),
            https_proxy: None,
            socks_proxy: None,
            no_proxy: None,
            source: ProxySource::GlobalConfig,
        };

        let exports = generate_shell_exports(&proxy, None);

        // Single quotes should be escaped
        assert!(exports.contains("'\\''"));
    }
}
