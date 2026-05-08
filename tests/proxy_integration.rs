//! Integration tests for proxy configuration parsing.
//!
//! These tests use Jarvy's typed `NetworkConfig` deserializer (not raw
//! `toml::Value`) so a regression in the `#[serde(...)]` annotations,
//! a renamed field, or a removed default is caught here — not by users
//! discovering their corp proxy stopped routing requests.

use jarvy::network::config::{NetworkConfig, NoProxy, PasswordSource};
use serde::Deserialize;

/// Wrapper that pulls just the `[network]` table from a `jarvy.toml` snippet.
#[derive(Debug, Deserialize)]
struct NetworkOnly {
    #[serde(default)]
    network: Option<NetworkConfig>,
}

fn parse(toml_text: &str) -> NetworkConfig {
    let parsed: NetworkOnly = toml::from_str(toml_text).expect("typed network deserialize");
    parsed.network.expect("expected [network] section")
}

#[test]
fn network_config_roundtrips_through_typed_deserializer() {
    let cfg = parse(
        r#"
        [network]
        https_proxy = "http://proxy.corp.com:8080"
        no_proxy = ["localhost", "127.0.0.1", ".corp.com"]

        [network.auth]
        username = "testuser"
        password = "testpass"

        [network.tls]
        ca_bundle = "/etc/ssl/certs/ca.crt"

        [network.overrides.git]
        https_proxy = "http://git-proxy.corp.com:8888"
        "#,
    );

    assert_eq!(
        cfg.https_proxy.as_deref(),
        Some("http://proxy.corp.com:8080")
    );
    match cfg.no_proxy.as_ref().expect("no_proxy parsed") {
        NoProxy::Array(items) => assert_eq!(items.len(), 3),
        NoProxy::String(_) => panic!("expected array form"),
    }
    let auth = cfg.auth.as_ref().expect("auth parsed");
    assert_eq!(auth.username, "testuser");
    let git = cfg.overrides.get("git").expect("git override deserialized");
    assert_eq!(
        git.https_proxy.as_deref(),
        Some("http://git-proxy.corp.com:8888")
    );
}

#[test]
fn no_proxy_string_form_deserializes() {
    let cfg = parse(
        r#"
        [network]
        https_proxy = "http://proxy.corp.com:8080"
        no_proxy = "localhost,127.0.0.1,.corp.com"
        "#,
    );
    match cfg.no_proxy.as_ref().expect("no_proxy parsed") {
        NoProxy::String(s) => assert_eq!(s, "localhost,127.0.0.1,.corp.com"),
        NoProxy::Array(_) => panic!("expected string form"),
    }
}

#[test]
fn minimal_network_config_uses_defaults() {
    let cfg = parse(
        r#"
        [network]
        https_proxy = "http://proxy.corp.com:8080"
        "#,
    );
    assert_eq!(
        cfg.https_proxy.as_deref(),
        Some("http://proxy.corp.com:8080")
    );
    assert!(cfg.http_proxy.is_none());
    assert!(cfg.no_proxy.is_none());
    assert!(cfg.auth.is_none());
    assert!(cfg.tls.is_none());
    assert!(cfg.overrides.is_empty());
}

#[test]
fn missing_network_section_deserializes_to_none() {
    let parsed: NetworkOnly = toml::from_str(
        r#"
        [provisioner]
        git = "latest"
        "#,
    )
    .expect("typed deserialize");
    assert!(parsed.network.is_none());
}

#[test]
fn auth_password_plain_form_deserializes() {
    let cfg = parse(
        r#"
        [network]
        https_proxy = "http://proxy.corp.com:8080"

        [network.auth]
        username = "jdoe"
        password = "PROXY_PASSWORD"
        "#,
    );
    let auth = cfg.auth.as_ref().expect("auth parsed");
    assert_eq!(auth.username, "jdoe");
    assert!(matches!(auth.password, PasswordSource::Plain(ref p) if p == "PROXY_PASSWORD"));
}

#[test]
fn tool_overrides_deserialize_with_typed_fields() {
    let cfg = parse(
        r#"
        [network]
        https_proxy = "http://proxy.corp.com:8080"

        [network.overrides.git]
        https_proxy = "http://git-proxy.corp.com:8888"

        [network.overrides.npm]
        no_proxy_all = true
        "#,
    );

    let git = cfg.overrides.get("git").expect("git override");
    assert_eq!(
        git.https_proxy.as_deref(),
        Some("http://git-proxy.corp.com:8888")
    );
    assert!(!git.no_proxy_all);

    let npm = cfg.overrides.get("npm").expect("npm override");
    assert!(npm.no_proxy_all);
    assert!(npm.https_proxy.is_none());
}

#[test]
fn tls_config_deserializes_with_typed_bool() {
    let cfg = parse(
        r#"
        [network]
        https_proxy = "http://proxy.corp.com:8080"

        [network.tls]
        ca_bundle = "/path/to/ca-bundle.crt"
        insecure = false
        "#,
    );
    let tls = cfg.tls.as_ref().expect("tls parsed");
    assert_eq!(tls.ca_bundle.as_deref(), Some("/path/to/ca-bundle.crt"));
    assert!(!tls.insecure);
}

#[test]
fn socks_proxy_deserializes() {
    let cfg = parse(
        r#"
        [network]
        socks_proxy = "socks5://proxy.corp.com:1080"
        "#,
    );
    assert_eq!(
        cfg.socks_proxy.as_deref(),
        Some("socks5://proxy.corp.com:1080")
    );
}

#[test]
fn combined_http_and_https_proxies_deserialize() {
    let cfg = parse(
        r#"
        [network]
        http_proxy = "http://proxy.corp.com:8080"
        https_proxy = "http://proxy.corp.com:8443"
        no_proxy = "localhost,127.0.0.1"
        "#,
    );
    assert_eq!(
        cfg.http_proxy.as_deref(),
        Some("http://proxy.corp.com:8080")
    );
    assert_eq!(
        cfg.https_proxy.as_deref(),
        Some("http://proxy.corp.com:8443")
    );
}
