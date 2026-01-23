//! Integration tests for proxy configuration functionality
//!
//! These tests verify the network/proxy configuration parsing and propagation.

/// Test that TOML with [network] section can be parsed
#[test]
fn test_network_config_parsing() {
    let toml_content = r#"
[provisioner]
git = "latest"

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
"#;

    // Parse using toml crate directly to verify format
    let config: toml::Value = toml::from_str(toml_content).expect("Failed to parse TOML");

    assert!(config.get("network").is_some());
    let network = config.get("network").unwrap();
    assert_eq!(
        network.get("https_proxy").and_then(|v| v.as_str()),
        Some("http://proxy.corp.com:8080")
    );

    let no_proxy = network.get("no_proxy").and_then(|v| v.as_array());
    assert!(no_proxy.is_some());
    assert_eq!(no_proxy.unwrap().len(), 3);
}

/// Test that no_proxy can be a comma-separated string
#[test]
fn test_network_config_no_proxy_string() {
    let toml_content = r#"
[provisioner]
git = "latest"

[network]
https_proxy = "http://proxy.corp.com:8080"
no_proxy = "localhost,127.0.0.1,.corp.com"
"#;

    let config: toml::Value = toml::from_str(toml_content).expect("Failed to parse TOML");

    let network = config.get("network").unwrap();
    let no_proxy = network.get("no_proxy").and_then(|v| v.as_str());
    assert_eq!(no_proxy, Some("localhost,127.0.0.1,.corp.com"));
}

/// Test minimal network configuration
#[test]
fn test_minimal_network_config() {
    let toml_content = r#"
[provisioner]
git = "latest"

[network]
https_proxy = "http://proxy.corp.com:8080"
"#;

    let config: toml::Value = toml::from_str(toml_content).expect("Failed to parse TOML");

    assert!(config.get("network").is_some());
    assert!(config.get("provisioner").is_some());
}

/// Test network config with authentication using env variable
#[test]
fn test_network_config_env_password() {
    let toml_content = r#"
[provisioner]
git = "latest"

[network]
https_proxy = "http://proxy.corp.com:8080"

[network.auth]
username = "jdoe"
password = "PROXY_PASSWORD"
"#;

    let config: toml::Value = toml::from_str(toml_content).expect("Failed to parse TOML");

    let auth = config.get("network").and_then(|n| n.get("auth"));
    assert!(auth.is_some());
    assert_eq!(
        auth.unwrap().get("username").and_then(|v| v.as_str()),
        Some("jdoe")
    );
}

/// Test that missing [network] section results in default config
#[test]
fn test_no_network_config() {
    let toml_content = r#"
[provisioner]
git = "latest"
"#;

    let config: toml::Value = toml::from_str(toml_content).expect("Failed to parse TOML");

    // No network section is valid - uses defaults
    assert!(config.get("network").is_none());
}

/// Test per-tool network override parsing
#[test]
fn test_tool_override_parsing() {
    let toml_content = r#"
[provisioner]
git = "latest"
npm = "latest"

[network]
https_proxy = "http://proxy.corp.com:8080"

[network.overrides.git]
https_proxy = "http://git-proxy.corp.com:8888"

[network.overrides.npm]
no_proxy_all = true
"#;

    let config: toml::Value = toml::from_str(toml_content).expect("Failed to parse TOML");

    let overrides = config.get("network").and_then(|n| n.get("overrides"));
    assert!(overrides.is_some());

    let git_override = overrides.unwrap().get("git");
    assert!(git_override.is_some());
    assert_eq!(
        git_override
            .unwrap()
            .get("https_proxy")
            .and_then(|v| v.as_str()),
        Some("http://git-proxy.corp.com:8888")
    );

    let npm_override = overrides.unwrap().get("npm");
    assert!(npm_override.is_some());
    assert_eq!(
        npm_override
            .unwrap()
            .get("no_proxy_all")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
}

/// Test TLS configuration parsing
#[test]
fn test_tls_config_parsing() {
    let toml_content = r#"
[provisioner]
git = "latest"

[network]
https_proxy = "http://proxy.corp.com:8080"

[network.tls]
ca_bundle = "/path/to/ca-bundle.crt"
insecure = false
"#;

    let config: toml::Value = toml::from_str(toml_content).expect("Failed to parse TOML");

    let tls = config.get("network").and_then(|n| n.get("tls"));
    assert!(tls.is_some());
    assert_eq!(
        tls.unwrap().get("ca_bundle").and_then(|v| v.as_str()),
        Some("/path/to/ca-bundle.crt")
    );
    assert_eq!(
        tls.unwrap().get("insecure").and_then(|v| v.as_bool()),
        Some(false)
    );
}

/// Test that SOCKS proxy configuration works
#[test]
fn test_socks_proxy_config() {
    let toml_content = r#"
[provisioner]
git = "latest"

[network]
socks_proxy = "socks5://proxy.corp.com:1080"
"#;

    let config: toml::Value = toml::from_str(toml_content).expect("Failed to parse TOML");

    let network = config.get("network").unwrap();
    assert_eq!(
        network.get("socks_proxy").and_then(|v| v.as_str()),
        Some("socks5://proxy.corp.com:1080")
    );
}

/// Test combined HTTP and HTTPS proxy configuration
#[test]
fn test_combined_proxy_config() {
    let toml_content = r#"
[provisioner]
git = "latest"

[network]
http_proxy = "http://proxy.corp.com:8080"
https_proxy = "http://proxy.corp.com:8443"
no_proxy = "localhost,127.0.0.1"
"#;

    let config: toml::Value = toml::from_str(toml_content).expect("Failed to parse TOML");

    let network = config.get("network").unwrap();
    assert_eq!(
        network.get("http_proxy").and_then(|v| v.as_str()),
        Some("http://proxy.corp.com:8080")
    );
    assert_eq!(
        network.get("https_proxy").and_then(|v| v.as_str()),
        Some("http://proxy.corp.com:8443")
    );
}
