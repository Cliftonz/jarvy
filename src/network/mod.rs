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

// Public API exports - these modules are part of the network module's interface
#[allow(unused_imports)]
pub use auth::*;
pub use config::*;
#[allow(unused_imports)]
pub use package_managers::*;
#[allow(unused_imports)]
pub use propagate::*;
#[allow(unused_imports)]
pub use resolve::*;
#[allow(unused_imports)]
pub use testing::*;

/// Redact home-directory prefix from a path string for safe logging/telemetry.
///
/// Replaces a leading `$HOME` with `~` so paths that contain user names do
/// not leak into shared ticket bundles or remote telemetry sinks.
#[allow(dead_code)] // Public API for safe path logging
pub fn redact_home(path: &str) -> String {
    let home = dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    if !home.is_empty() && path.starts_with(&home) {
        path.replacen(&home, "~", 1)
    } else {
        path.to_string()
    }
}

/// Redact credentials from a proxy or git URL for safe logging.
///
/// Handles three credential shapes that appear in real configs:
/// - `scheme://user:password@host` → `scheme://user:***@host`
/// - `scheme://token@host` (userinfo-only token, e.g. `https://ghp_xxx@github.com`)
///   → `scheme://***@host`
/// - `scheme://:password@host` (password-only) → `scheme://:***@host`
///
/// Returns `Cow::Borrowed` when the URL has no `userinfo@` segment so callers
/// that log on the no-credentials path do not pay an allocation.
#[allow(dead_code)] // Public API for safe proxy URL logging
pub fn redact_credentials(url: &str) -> std::borrow::Cow<'_, str> {
    let Some((proto_part, creds_part, after_at)) = split_userinfo(url) else {
        return std::borrow::Cow::Borrowed(url);
    };

    // No `:` => userinfo-only (e.g. token-as-username). Redact the whole creds.
    let redacted = match creds_part.find(':') {
        Some(colon) => {
            let username = &creds_part[..colon];
            if username.is_empty() {
                format!("{}:***{}", proto_part, after_at)
            } else {
                format!("{}{}:***{}", proto_part, username, after_at)
            }
        }
        None => format!("{}***{}", proto_part, after_at),
    };
    std::borrow::Cow::Owned(redacted)
}

/// Like [`redact_credentials`], but masks the whole userinfo (username
/// included) for telemetry sinks: `scheme://user:pass@host` → `scheme://***@host`.
pub fn redact_userinfo(url: &str) -> std::borrow::Cow<'_, str> {
    match split_userinfo(url) {
        Some((proto_part, _, after_at)) => {
            std::borrow::Cow::Owned(format!("{}***{}", proto_part, after_at))
        }
        None => std::borrow::Cow::Borrowed(url),
    }
}

/// Splits `url` into `(scheme://, userinfo, @rest)` when the authority
/// carries a userinfo segment. Only an `@` before the first `/`, `?`, or `#`
/// after the scheme counts, so `@` in paths or queries is never mistaken for it.
fn split_userinfo(url: &str) -> Option<(&str, &str, &str)> {
    let proto_end = url.find("://")? + 3;
    let after_scheme = &url[proto_end..];
    let authority_end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    let at = after_scheme[..authority_end].find('@')?;
    Some((
        &url[..proto_end],
        &after_scheme[..at],
        &url[proto_end + at..],
    ))
}

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
    fn redact_userinfo_only_token() {
        // GitHub-style PATs are passed as the userinfo with no password.
        let url = "https://ghp_abcd1234@github.com/owner/repo.git";
        let redacted = redact_credentials(url);
        assert_eq!(
            &*redacted, "https://***@github.com/owner/repo.git",
            "userinfo-only token must be fully redacted"
        );
        assert!(!redacted.contains("ghp_abcd1234"));
    }

    #[test]
    fn redact_password_only() {
        let url = "http://:secret@host:8080";
        let redacted = redact_credentials(url);
        assert_eq!(&*redacted, "http://:***@host:8080");
    }

    #[test]
    fn redact_no_credentials_borrows() {
        let url = "https://github.com/owner/repo.git";
        let redacted = redact_credentials(url);
        assert!(matches!(redacted, std::borrow::Cow::Borrowed(_)));
        assert_eq!(&*redacted, url);
    }

    #[test]
    fn redact_does_not_target_at_in_path() {
        // The `@` in the path/query must not be mistaken for userinfo.
        let url = "https://github.com/owner/repo/blob/main/file.txt?ref=abc@v1";
        let redacted = redact_credentials(url);
        assert_eq!(&*redacted, url);
    }

    #[test]
    fn redact_userinfo_masks_username_and_password() {
        let redacted = redact_userinfo("https://alice:secret@example.com/r");
        assert!(!redacted.contains("alice"));
        assert!(!redacted.contains("secret"));
        assert!(redacted.contains("example.com/r"));
    }

    #[test]
    fn redact_userinfo_without_userinfo_is_unchanged() {
        let url = "https://example.com/r";
        assert_eq!(&*redact_userinfo(url), url);
    }

    #[test]
    fn redact_socks5() {
        let url = "socks5://u:p@host:1080";
        let redacted = redact_credentials(url);
        assert_eq!(&*redacted, "socks5://u:***@host:1080");
    }

    #[test]
    fn redact_unparseable_returns_original() {
        // No scheme => not a URL we can confidently parse; return as-is.
        let weird = "not://valid".replace("://", "");
        let redacted = redact_credentials(&weird);
        assert_eq!(&*redacted, &weird);
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
