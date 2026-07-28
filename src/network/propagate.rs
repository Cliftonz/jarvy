//! Proxy propagation to child processes and environment variables
//!
//! This module handles applying proxy settings to spawned commands and
//! generating environment variables for different tools.

#![allow(dead_code)] // Public API for proxy propagation

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

/// Directories whose CA bundles we trust by default. A `[network.tls]
/// ca_bundle` outside these requires explicit opt-in via
/// `JARVY_ALLOW_CUSTOM_CA=1` because the file replaces the default trust
/// store of every spawned tool (npm registry, git clone, pip install,
/// aws cli) and silently MitMs all subsequent installs.
const TRUSTED_CA_BUNDLE_DIRS: &[&str] = &[
    "/etc/ssl/",
    "/etc/pki/",
    "/etc/ca-certificates/",
    "/usr/share/ca-certificates/",
    "/usr/local/share/ca-certificates/",
    "/opt/homebrew/etc/ca-certificates/",
    "/opt/homebrew/share/ca-certificates/",
    // Library/Keychains is an opt-in path for macOS but worth allowing.
    "/Library/Keychains/",
    "/System/Library/Keychains/",
];

/// Returns true if `path` is acceptable as a CA bundle without explicit
/// opt-in: the file lives under a trusted system root, OR it lives under
/// `~/.jarvy/ca/` (a Jarvy-curated dir we never auto-write to), OR
/// `JARVY_ALLOW_CUSTOM_CA=1` has been set. Hostile `/tmp/attacker.crt`
/// paths from a project `jarvy.toml` fail this check.
///
/// **Why `~/.jarvy/ca/` and not `~/.jarvy/` writ large?** Round-2 review
/// found a chain: a hostile remote `jarvy.toml` is cached at
/// `~/.jarvy/cache/configs/<sha>.toml` (attacker-controlled bytes). If
/// the user can then point `[network.tls] ca_bundle` at that cached
/// file — which the broad `~/.jarvy/` prefix used to allow — OpenSSL,
/// curl, and rustls happily extract `BEGIN CERTIFICATE` blocks out of
/// it and trust the embedded root, MitMing every subsequent
/// `git clone` / `npm install` / `brew install` / `pip install`. The
/// narrow `~/.jarvy/ca/` dir is one Jarvy never writes to as part of
/// any auto flow, so attacker-controlled bytes cannot land there
/// without explicit user action.
fn ca_bundle_path_is_trusted(path: &str) -> bool {
    use std::path::{Path, PathBuf};

    if std::env::var("JARVY_ALLOW_CUSTOM_CA")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"))
        .unwrap_or(false)
    {
        return true;
    }
    let canon: PathBuf = std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path));
    let canon_str = canon.to_string_lossy();
    if TRUSTED_CA_BUNDLE_DIRS
        .iter()
        .any(|dir| canon_str.starts_with(dir))
    {
        return true;
    }
    if let Ok(jarvy_dir) = crate::paths::jarvy_home() {
        let curated_ca_dir = jarvy_dir.join("ca");
        // `Path::starts_with` is component-aware and cross-platform —
        // rejects `~/.jarvy/ca-attacker/` (sibling-prefix attack) and
        // honors backslash separators on Windows. Earlier
        // `format!("{prefix}/")` shape was Unix-only and silently
        // hard-failed every Windows tag-push CI run since v0.2.0-rc.1.
        if Path::new(&canon).starts_with(&curated_ca_dir) {
            return true;
        }
    }
    false
}

/// Generate all environment variables for proxy and TLS settings.
///
/// Untrusted `ca_bundle` paths are silently dropped from the propagated
/// env (a `tracing::warn!` event is emitted). This means a hostile
/// `[network.tls] ca_bundle = "/tmp/attacker.crt"` produces a setup that
/// runs WITHOUT a CA override, not one that trusts the attacker's CA —
/// fail-safe direction for MitM exposure.
pub fn generate_env_vars(
    proxy: &ResolvedProxy,
    tls: Option<&TlsConfig>,
) -> HashMap<String, String> {
    let mut vars = proxy.to_env_vars();

    // Add CA bundle variables if configured AND the path is trusted.
    if let Some(tls_config) = tls {
        if tls_config.insecure {
            tracing::warn!(
                event = "network.tls.insecure_ignored",
                "[network.tls] insecure=true is parsed but never applied; \
                 jarvy refuses to disable TLS verification from project config"
            );
        }
        if let Some(ref ca_bundle) = tls_config.ca_bundle {
            if ca_bundle_path_is_trusted(ca_bundle) {
                vars.insert(env_keys::CURL_CA_BUNDLE.to_string(), ca_bundle.clone());
                vars.insert(env_keys::SSL_CERT_FILE.to_string(), ca_bundle.clone());
                vars.insert(env_keys::REQUESTS_CA_BUNDLE.to_string(), ca_bundle.clone());
                vars.insert(env_keys::NODE_EXTRA_CA_CERTS.to_string(), ca_bundle.clone());
                vars.insert(env_keys::GIT_SSL_CAINFO.to_string(), ca_bundle.clone());
                vars.insert(env_keys::AWS_CA_BUNDLE.to_string(), ca_bundle.clone());
            } else {
                tracing::warn!(
                    event = "network.tls.refused_untrusted_ca_bundle",
                    path = %ca_bundle,
                    "refused [network.tls] ca_bundle outside trusted dirs; \
                     set JARVY_ALLOW_CUSTOM_CA=1 to override"
                );
            }
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
        // Use a path under /etc/ssl/ which is in TRUSTED_CA_BUNDLE_DIRS.
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
    fn ca_bundle_outside_trusted_dirs_is_dropped() {
        // SAFETY: scoped to this test; if JARVY_ALLOW_CUSTOM_CA happens to be
        // set we skip rather than corrupt env.
        if std::env::var("JARVY_ALLOW_CUSTOM_CA").is_ok() {
            return;
        }
        let proxy = ResolvedProxy::default();
        let tls = TlsConfig {
            ca_bundle: Some("/tmp/attacker.crt".to_string()),
            insecure: false,
        };
        let vars = generate_env_vars(&proxy, Some(&tls));
        assert!(
            !vars.contains_key("CURL_CA_BUNDLE"),
            "untrusted ca_bundle must NOT be propagated; got {:?}",
            vars.get("CURL_CA_BUNDLE")
        );
        assert!(!vars.contains_key("SSL_CERT_FILE"));
        assert!(!vars.contains_key("NODE_EXTRA_CA_CERTS"));
    }

    #[test]
    fn ca_bundle_path_predicate_accepts_system_dirs() {
        assert!(ca_bundle_path_is_trusted("/etc/ssl/certs/ca-bundle.crt"));
        assert!(ca_bundle_path_is_trusted(
            "/etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem"
        ));
        assert!(ca_bundle_path_is_trusted(
            "/usr/share/ca-certificates/cacert.org/cacert.org.crt"
        ));
    }

    #[test]
    fn ca_bundle_path_predicate_rejects_user_writable_paths() {
        if std::env::var("JARVY_ALLOW_CUSTOM_CA").is_ok() {
            return;
        }
        assert!(!ca_bundle_path_is_trusted("/tmp/attacker.crt"));
        assert!(!ca_bundle_path_is_trusted("/var/tmp/x"));
    }

    // Dual serial keys: these tests call `jarvy_home()` twice (directly
    // + inside the predicate). `jarvy_test_home` guards against init.rs /
    // maintenance::lock tests flipping `JARVY_TEST_HOME`; `jarvy_home_env`
    // guards against paths.rs / agent_profiles tests flipping `JARVY_HOME`.
    // A mid-test flip of either makes the two prefixes disagree.
    #[test]
    #[serial_test::serial(jarvy_test_home, jarvy_home_env)]
    fn ca_bundle_path_under_jarvy_cache_is_refused() {
        // Round-2 P0: the cached body of a hostile remote `jarvy.toml`
        // lives at `~/.jarvy/cache/configs/<sha>.toml`. The broad
        // `~/.jarvy/` prefix used to make this attacker-controlled file
        // a valid CA bundle. Tighten verified.
        if std::env::var("JARVY_ALLOW_CUSTOM_CA").is_ok() {
            return;
        }
        if let Ok(jarvy_dir) = crate::paths::jarvy_home() {
            let cached = jarvy_dir.join("cache").join("configs").join("abc.toml");
            assert!(
                !ca_bundle_path_is_trusted(&cached.to_string_lossy()),
                "cache file under ~/.jarvy/ must not be a trusted CA bundle"
            );
            // Also reject sibling-prefix attacks like `~/.jarvy/ca-attacker/`.
            let sibling = jarvy_dir.join("ca-attacker").join("evil.crt");
            assert!(!ca_bundle_path_is_trusted(&sibling.to_string_lossy()));
        }
    }

    #[test]
    #[serial_test::serial(jarvy_test_home, jarvy_home_env)]
    fn ca_bundle_path_under_curated_ca_dir_is_trusted() {
        if std::env::var("JARVY_ALLOW_CUSTOM_CA").is_ok() {
            return;
        }
        if let Ok(jarvy_dir) = crate::paths::jarvy_home() {
            let curated = jarvy_dir.join("ca").join("corp.crt");
            // Use a string compare since the file may not exist; the
            // function falls back to the raw path when canonicalize fails.
            assert!(
                ca_bundle_path_is_trusted(&curated.to_string_lossy()),
                "~/.jarvy/ca/<file> must remain trusted (the dir Jarvy never auto-writes)"
            );
        }
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
