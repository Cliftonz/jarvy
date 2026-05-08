//! Proxy resolution with priority handling
//!
//! Priority order:
//! 1. Environment variables (HTTP_PROXY, HTTPS_PROXY, etc.)
//! 2. Tool-specific overrides in [network.overrides.<tool>]
//! 3. Global config in [network] section

#![allow(dead_code)] // Public API for proxy resolution

use super::config::{NetworkConfig, NetworkOverride};
use std::collections::HashMap;
use std::env;

/// Resolved proxy configuration for a specific tool
#[derive(Debug, Clone, Default)]
pub struct ResolvedProxy {
    pub http_proxy: Option<String>,
    pub https_proxy: Option<String>,
    pub socks_proxy: Option<String>,
    pub no_proxy: Option<String>,
    pub source: ProxySource,
}

/// Source of the resolved proxy configuration
#[derive(Debug, Clone, Default, PartialEq)]
pub enum ProxySource {
    #[default]
    None,
    Environment,
    ToolOverride(String),
    GlobalConfig,
}

/// Proxy resolver that combines environment, config, and tool-specific settings
pub struct ProxyResolver<'a> {
    config: Option<&'a NetworkConfig>,
}

impl<'a> ProxyResolver<'a> {
    /// Create a new proxy resolver
    pub fn new(config: Option<&'a NetworkConfig>) -> Self {
        Self { config }
    }

    /// Resolve proxy configuration for a specific tool
    pub fn resolve_for_tool(&self, tool_name: &str) -> ResolvedProxy {
        // Check environment variables first (highest priority)
        if let Some(proxy) = self.env_proxy() {
            return proxy;
        }

        // Check tool-specific override
        if let Some(config) = self.config {
            if let Some(override_config) = config.overrides.get(tool_name) {
                if override_config.no_proxy_all {
                    // Tool explicitly disables proxy
                    return ResolvedProxy {
                        source: ProxySource::ToolOverride(tool_name.to_string()),
                        ..Default::default()
                    };
                }

                let resolved = self.override_proxy(override_config, tool_name);
                if resolved.http_proxy.is_some() || resolved.https_proxy.is_some() {
                    return resolved;
                }
            }
        }

        // Fall back to global config
        self.global_proxy()
    }

    /// Get proxy from environment variables
    fn env_proxy(&self) -> Option<ResolvedProxy> {
        let http = env::var("HTTP_PROXY")
            .or_else(|_| env::var("http_proxy"))
            .ok();
        let https = env::var("HTTPS_PROXY")
            .or_else(|_| env::var("https_proxy"))
            .ok();
        let socks = env::var("SOCKS_PROXY")
            .or_else(|_| env::var("socks_proxy"))
            .or_else(|_| env::var("ALL_PROXY"))
            .or_else(|_| env::var("all_proxy"))
            .ok();
        let no_proxy = env::var("NO_PROXY").or_else(|_| env::var("no_proxy")).ok();

        if http.is_some() || https.is_some() || socks.is_some() {
            Some(ResolvedProxy {
                http_proxy: http,
                https_proxy: https,
                socks_proxy: socks,
                no_proxy,
                source: ProxySource::Environment,
            })
        } else {
            None
        }
    }

    /// Get proxy from tool-specific override
    fn override_proxy(&self, override_config: &NetworkOverride, tool_name: &str) -> ResolvedProxy {
        let global = self.config;

        ResolvedProxy {
            http_proxy: override_config
                .http_proxy
                .clone()
                .or_else(|| global.and_then(|c| c.http_proxy.clone())),
            https_proxy: override_config
                .https_proxy
                .clone()
                .or_else(|| global.and_then(|c| c.https_proxy.clone())),
            socks_proxy: override_config
                .socks_proxy
                .clone()
                .or_else(|| global.and_then(|c| c.socks_proxy.clone())),
            no_proxy: override_config
                .no_proxy
                .as_ref()
                .map(|np| np.to_env_string())
                .or_else(|| global.and_then(|c| c.no_proxy.as_ref().map(|np| np.to_env_string()))),
            source: ProxySource::ToolOverride(tool_name.to_string()),
        }
    }

    /// Get proxy from global config
    fn global_proxy(&self) -> ResolvedProxy {
        match self.config {
            Some(config) if config.has_proxy() => ResolvedProxy {
                http_proxy: config.http_proxy.clone(),
                https_proxy: config.https_proxy.clone(),
                socks_proxy: config.socks_proxy.clone(),
                no_proxy: config.no_proxy.as_ref().map(|np| np.to_env_string()),
                source: ProxySource::GlobalConfig,
            },
            _ => ResolvedProxy::default(),
        }
    }
}

impl ResolvedProxy {
    /// Check if any proxy is configured
    pub fn has_proxy(&self) -> bool {
        self.http_proxy.is_some() || self.https_proxy.is_some() || self.socks_proxy.is_some()
    }

    /// Convert to environment variable HashMap
    pub fn to_env_vars(&self) -> HashMap<String, String> {
        let mut vars = HashMap::new();

        if let Some(ref proxy) = self.http_proxy {
            vars.insert("HTTP_PROXY".to_string(), proxy.clone());
            vars.insert("http_proxy".to_string(), proxy.clone());
        }

        if let Some(ref proxy) = self.https_proxy {
            vars.insert("HTTPS_PROXY".to_string(), proxy.clone());
            vars.insert("https_proxy".to_string(), proxy.clone());
        }

        if let Some(ref proxy) = self.socks_proxy {
            vars.insert("SOCKS_PROXY".to_string(), proxy.clone());
            vars.insert("socks_proxy".to_string(), proxy.clone());
            vars.insert("ALL_PROXY".to_string(), proxy.clone());
            vars.insert("all_proxy".to_string(), proxy.clone());
        }

        if let Some(ref no_proxy) = self.no_proxy {
            vars.insert("NO_PROXY".to_string(), no_proxy.clone());
            vars.insert("no_proxy".to_string(), no_proxy.clone());
        }

        vars
    }
}

#[cfg(test)]
#[allow(unsafe_code)]
mod tests {
    use super::*;

    #[test]
    fn test_resolver_no_config() {
        let resolver = ProxyResolver::new(None);
        let resolved = resolver.resolve_for_tool("git");
        assert!(!resolved.has_proxy());
        assert_eq!(resolved.source, ProxySource::None);
    }

    #[test]
    fn test_resolver_global_config() {
        let config = NetworkConfig {
            https_proxy: Some("http://proxy:8080".to_string()),
            ..Default::default()
        };

        let resolver = ProxyResolver::new(Some(&config));
        let resolved = resolver.resolve_for_tool("git");

        assert!(resolved.has_proxy());
        assert_eq!(resolved.https_proxy, Some("http://proxy:8080".to_string()));
        assert_eq!(resolved.source, ProxySource::GlobalConfig);
    }

    #[test]
    fn test_resolver_tool_override() {
        let git_override = NetworkOverride {
            https_proxy: Some("http://git-proxy:8888".to_string()),
            ..Default::default()
        };
        let mut overrides = std::collections::HashMap::new();
        overrides.insert("git".to_string(), git_override);
        let config = NetworkConfig {
            https_proxy: Some("http://proxy:8080".to_string()),
            overrides,
            ..Default::default()
        };

        let resolver = ProxyResolver::new(Some(&config));
        let resolved = resolver.resolve_for_tool("git");

        assert_eq!(
            resolved.https_proxy,
            Some("http://git-proxy:8888".to_string())
        );
        assert_eq!(
            resolved.source,
            ProxySource::ToolOverride("git".to_string())
        );
    }

    #[test]
    fn test_resolver_tool_no_proxy_all() {
        let git_override = NetworkOverride {
            no_proxy_all: true,
            ..Default::default()
        };
        let mut overrides = std::collections::HashMap::new();
        overrides.insert("git".to_string(), git_override);
        let config = NetworkConfig {
            https_proxy: Some("http://proxy:8080".to_string()),
            overrides,
            ..Default::default()
        };

        let resolver = ProxyResolver::new(Some(&config));
        let resolved = resolver.resolve_for_tool("git");

        assert!(!resolved.has_proxy());
        assert_eq!(
            resolved.source,
            ProxySource::ToolOverride("git".to_string())
        );
    }

    /// RAII guard that snapshots the listed env vars on construction and
    /// restores them on drop. Pair with `#[serial(proxy_env)]` so concurrent
    /// tests do not race on the global env.
    struct EnvGuard {
        saved: Vec<(&'static str, Option<String>)>,
    }

    impl EnvGuard {
        fn new(vars: &[&'static str]) -> Self {
            let saved = vars.iter().map(|k| (*k, std::env::var(k).ok())).collect();
            // Wipe each so tests start from a known state.
            for k in vars {
                // SAFETY: tests run with #[serial(proxy_env)] so no other
                // thread is reading or writing these vars concurrently.
                unsafe { std::env::remove_var(k) };
            }
            Self { saved }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (k, v) in &self.saved {
                // SAFETY: see EnvGuard::new.
                unsafe {
                    match v {
                        Some(val) => std::env::set_var(k, val),
                        None => std::env::remove_var(k),
                    }
                }
            }
        }
    }

    const PROXY_ENV_VARS: &[&str] = &[
        "HTTP_PROXY",
        "http_proxy",
        "HTTPS_PROXY",
        "https_proxy",
        "SOCKS_PROXY",
        "socks_proxy",
        "ALL_PROXY",
        "all_proxy",
        "NO_PROXY",
        "no_proxy",
    ];

    #[test]
    #[serial_test::serial(proxy_env)]
    fn proxy_precedence_env_beats_tool_and_global() {
        let _guard = EnvGuard::new(PROXY_ENV_VARS);
        // SAFETY: serialized via #[serial(proxy_env)].
        unsafe { std::env::set_var("HTTPS_PROXY", "https://env.example/x") };

        let git_override = NetworkOverride {
            https_proxy: Some("https://tool.example/y".to_string()),
            ..Default::default()
        };
        let mut overrides = std::collections::HashMap::new();
        overrides.insert("git".to_string(), git_override);
        let config = NetworkConfig {
            https_proxy: Some("https://global.example/z".to_string()),
            overrides,
            ..Default::default()
        };

        let resolver = ProxyResolver::new(Some(&config));
        let resolved = resolver.resolve_for_tool("git");

        assert_eq!(resolved.source, ProxySource::Environment);
        assert_eq!(
            resolved.https_proxy.as_deref(),
            Some("https://env.example/x")
        );
    }

    #[test]
    #[serial_test::serial(proxy_env)]
    fn proxy_precedence_tool_beats_global_when_no_env() {
        let _guard = EnvGuard::new(PROXY_ENV_VARS);

        let git_override = NetworkOverride {
            https_proxy: Some("https://tool.example/y".to_string()),
            ..Default::default()
        };
        let mut overrides = std::collections::HashMap::new();
        overrides.insert("git".to_string(), git_override);
        let config = NetworkConfig {
            https_proxy: Some("https://global.example/z".to_string()),
            overrides,
            ..Default::default()
        };

        let resolver = ProxyResolver::new(Some(&config));
        let resolved = resolver.resolve_for_tool("git");

        assert_eq!(
            resolved.source,
            ProxySource::ToolOverride("git".to_string())
        );
        assert_eq!(
            resolved.https_proxy.as_deref(),
            Some("https://tool.example/y")
        );
    }

    #[test]
    #[serial_test::serial(proxy_env)]
    fn proxy_no_proxy_all_drops_global_proxy() {
        let _guard = EnvGuard::new(PROXY_ENV_VARS);

        let git_override = NetworkOverride {
            no_proxy_all: true,
            ..Default::default()
        };
        let mut overrides = std::collections::HashMap::new();
        overrides.insert("git".to_string(), git_override);
        let config = NetworkConfig {
            https_proxy: Some("https://global.example/z".to_string()),
            overrides,
            ..Default::default()
        };

        let resolver = ProxyResolver::new(Some(&config));
        let resolved = resolver.resolve_for_tool("git");

        assert!(!resolved.has_proxy());
        assert_eq!(
            resolved.source,
            ProxySource::ToolOverride("git".to_string())
        );
    }

    #[test]
    #[serial_test::serial(proxy_env)]
    fn proxy_unrelated_tool_falls_back_to_global() {
        let _guard = EnvGuard::new(PROXY_ENV_VARS);

        let git_override = NetworkOverride {
            https_proxy: Some("https://git.example/y".to_string()),
            ..Default::default()
        };
        let mut overrides = std::collections::HashMap::new();
        overrides.insert("git".to_string(), git_override);
        let config = NetworkConfig {
            https_proxy: Some("https://global.example/z".to_string()),
            overrides,
            ..Default::default()
        };

        let resolver = ProxyResolver::new(Some(&config));
        let resolved = resolver.resolve_for_tool("npm");

        assert_eq!(resolved.source, ProxySource::GlobalConfig);
        assert_eq!(
            resolved.https_proxy.as_deref(),
            Some("https://global.example/z")
        );
    }

    #[test]
    fn test_resolved_proxy_to_env_vars() {
        let proxy = ResolvedProxy {
            http_proxy: Some("http://proxy:8080".to_string()),
            https_proxy: Some("https://proxy:8443".to_string()),
            socks_proxy: None,
            no_proxy: Some("localhost,127.0.0.1".to_string()),
            source: ProxySource::GlobalConfig,
        };

        let vars = proxy.to_env_vars();
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
        assert_eq!(
            vars.get("NO_PROXY"),
            Some(&"localhost,127.0.0.1".to_string())
        );
    }
}
