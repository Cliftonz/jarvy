//! Package manager-specific proxy configuration helpers
//!
//! Different package managers require different configuration methods:
//! - Some respect environment variables (npm, pip, cargo)
//! - Some need explicit config commands (git, apt)
//! - Some need config files (apt, dnf)

use super::resolve::ResolvedProxy;
use std::collections::HashMap;
use std::process::Command;

/// Configure git proxy settings via git config
pub fn configure_git_proxy(proxy: &ResolvedProxy) -> Vec<String> {
    let mut commands = Vec::new();

    if let Some(ref http_proxy) = proxy.http_proxy {
        commands.push(format!("git config --global http.proxy {}", http_proxy));
    }

    if let Some(ref https_proxy) = proxy.https_proxy {
        commands.push(format!("git config --global https.proxy {}", https_proxy));
    }

    if let Some(ref no_proxy) = proxy.no_proxy {
        // Git doesn't have a direct no_proxy config, but we can set proxy for specific hosts
        for host in no_proxy.split(',') {
            let host = host.trim();
            if !host.is_empty() {
                commands.push(format!("git config --global http.{}.proxy \"\"", host));
            }
        }
    }

    commands
}

/// Remove git proxy settings
pub fn clear_git_proxy() -> Vec<String> {
    vec![
        "git config --global --unset http.proxy".to_string(),
        "git config --global --unset https.proxy".to_string(),
    ]
}

/// Configure npm proxy settings via npm config
pub fn configure_npm_proxy(proxy: &ResolvedProxy) -> Vec<String> {
    let mut commands = Vec::new();

    if let Some(ref http_proxy) = proxy.http_proxy {
        commands.push(format!("npm config set proxy {}", http_proxy));
    }

    if let Some(ref https_proxy) = proxy.https_proxy {
        commands.push(format!("npm config set https-proxy {}", https_proxy));
    }

    if let Some(ref no_proxy) = proxy.no_proxy {
        commands.push(format!("npm config set noproxy {}", no_proxy));
    }

    commands
}

/// Remove npm proxy settings
pub fn clear_npm_proxy() -> Vec<String> {
    vec![
        "npm config delete proxy".to_string(),
        "npm config delete https-proxy".to_string(),
        "npm config delete noproxy".to_string(),
    ]
}

/// Get Homebrew-specific environment variables
pub fn homebrew_env_vars(proxy: &ResolvedProxy) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    // Homebrew respects standard proxy env vars
    if let Some(ref http_proxy) = proxy.http_proxy {
        vars.insert("HOMEBREW_HTTP_PROXY".to_string(), http_proxy.clone());
    }

    if let Some(ref https_proxy) = proxy.https_proxy {
        vars.insert("HOMEBREW_HTTPS_PROXY".to_string(), https_proxy.clone());
    }

    if let Some(ref no_proxy) = proxy.no_proxy {
        vars.insert("HOMEBREW_NO_PROXY".to_string(), no_proxy.clone());
    }

    vars
}

/// Generate apt proxy configuration content
///
/// Returns content for /etc/apt/apt.conf.d/proxy.conf
/// Note: This requires sudo to write
pub fn apt_proxy_config(proxy: &ResolvedProxy) -> String {
    let mut lines = Vec::new();

    if let Some(ref http_proxy) = proxy.http_proxy {
        lines.push(format!("Acquire::http::Proxy \"{}\";", http_proxy));
    }

    if let Some(ref https_proxy) = proxy.https_proxy {
        lines.push(format!("Acquire::https::Proxy \"{}\";", https_proxy));
    }

    lines.join("\n")
}

/// Generate dnf/yum proxy configuration content
///
/// Returns content to append to /etc/dnf/dnf.conf or /etc/yum.conf
/// Note: This requires sudo to write
pub fn dnf_proxy_config(proxy: &ResolvedProxy) -> String {
    let mut lines = Vec::new();

    if let Some(ref https_proxy) = proxy.https_proxy {
        lines.push(format!("proxy={}", https_proxy));
    } else if let Some(ref http_proxy) = proxy.http_proxy {
        lines.push(format!("proxy={}", http_proxy));
    }

    lines.join("\n")
}

/// Documentation of proxy propagation behavior per tool
pub fn proxy_behavior_docs() -> &'static str {
    r#"
# Proxy Propagation by Tool

## Environment Variable Based (automatic)
- curl: HTTP_PROXY, HTTPS_PROXY, NO_PROXY, CURL_CA_BUNDLE
- wget: http_proxy, https_proxy, no_proxy
- pip: HTTP_PROXY, HTTPS_PROXY, NO_PROXY, REQUESTS_CA_BUNDLE
- cargo: HTTP_PROXY, HTTPS_PROXY
- node/npm: HTTP_PROXY, HTTPS_PROXY, NO_PROXY, NODE_EXTRA_CA_CERTS
- docker: HTTP_PROXY, HTTPS_PROXY, NO_PROXY

## Requires Explicit Configuration
- git: git config --global http.proxy / https.proxy
- npm: npm config set proxy / https-proxy (alternative to env vars)
- apt: /etc/apt/apt.conf.d/proxy.conf (requires sudo)
- dnf/yum: /etc/dnf/dnf.conf or /etc/yum.conf (requires sudo)

## Notes
- Most tools support both uppercase and lowercase env vars
- Some tools (apt, dnf) require config files with sudo access
- CA bundle propagation uses multiple env vars for broad compatibility
"#
}

/// Check if a tool needs explicit proxy configuration (not just env vars)
pub fn needs_explicit_config(tool: &str) -> bool {
    matches!(tool, "git" | "apt" | "dnf" | "yum")
}

#[cfg(test)]
mod tests {
    use super::super::resolve::ProxySource;
    use super::*;

    fn make_proxy() -> ResolvedProxy {
        ResolvedProxy {
            http_proxy: Some("http://proxy:8080".to_string()),
            https_proxy: Some("https://proxy:8443".to_string()),
            socks_proxy: None,
            no_proxy: Some("localhost,127.0.0.1".to_string()),
            source: ProxySource::GlobalConfig,
        }
    }

    #[test]
    fn test_configure_git_proxy() {
        let proxy = make_proxy();
        let commands = configure_git_proxy(&proxy);

        assert!(commands.iter().any(|c| c.contains("http.proxy")));
        assert!(commands.iter().any(|c| c.contains("https.proxy")));
    }

    #[test]
    fn test_configure_npm_proxy() {
        let proxy = make_proxy();
        let commands = configure_npm_proxy(&proxy);

        assert!(commands.iter().any(|c| c.contains("npm config set proxy")));
        assert!(
            commands
                .iter()
                .any(|c| c.contains("npm config set https-proxy"))
        );
    }

    #[test]
    fn test_homebrew_env_vars() {
        let proxy = make_proxy();
        let vars = homebrew_env_vars(&proxy);

        assert!(vars.contains_key("HOMEBREW_HTTP_PROXY"));
        assert!(vars.contains_key("HOMEBREW_HTTPS_PROXY"));
    }

    #[test]
    fn test_apt_proxy_config() {
        let proxy = make_proxy();
        let config = apt_proxy_config(&proxy);

        assert!(config.contains("Acquire::http::Proxy"));
        assert!(config.contains("Acquire::https::Proxy"));
    }

    #[test]
    fn test_needs_explicit_config() {
        assert!(needs_explicit_config("git"));
        assert!(needs_explicit_config("apt"));
        assert!(!needs_explicit_config("curl"));
        assert!(!needs_explicit_config("npm"));
    }
}
