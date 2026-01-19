//! Proxy connectivity testing
//!
//! Provides functionality to test proxy configuration and connectivity.

use super::config::{NetworkConfig, TlsConfig};
use super::resolve::{ProxyResolver, ResolvedProxy};
use std::time::Duration;

/// Result of a proxy connectivity test
#[derive(Debug, Clone)]
pub struct ProxyTestResult {
    pub proxy_url: String,
    pub success: bool,
    pub status_code: Option<u16>,
    pub error: Option<String>,
    pub response_time_ms: Option<u64>,
}

/// Test URLs for connectivity validation
pub const TEST_URLS: &[(&str, &str)] = &[
    ("GitHub", "https://api.github.com"),
    ("npm Registry", "https://registry.npmjs.org"),
    ("PyPI", "https://pypi.org/simple/"),
    ("Homebrew", "https://formulae.brew.sh"),
    ("crates.io", "https://crates.io/api/v1/crates"),
];

/// Test proxy connectivity by making HTTP requests through the proxy
///
/// Note: This is a simplified test that checks if the proxy is reachable.
/// Full implementation would use ureq or reqwest with proxy support.
pub fn test_proxy_connectivity(config: &NetworkConfig) -> Vec<ProxyTestResult> {
    let mut results = Vec::new();
    let resolver = ProxyResolver::new(Some(config));
    let resolved = resolver.resolve_for_tool("test");

    // Test HTTP proxy
    if let Some(ref proxy) = resolved.http_proxy {
        results.push(test_single_proxy(proxy, "http"));
    }

    // Test HTTPS proxy
    if let Some(ref proxy) = resolved.https_proxy {
        results.push(test_single_proxy(proxy, "https"));
    }

    // Test SOCKS proxy
    if let Some(ref proxy) = resolved.socks_proxy {
        results.push(test_single_proxy(proxy, "socks"));
    }

    results
}

/// Test a single proxy URL
fn test_single_proxy(proxy_url: &str, proxy_type: &str) -> ProxyTestResult {
    // Parse proxy URL to extract host and port
    let result = parse_proxy_url(proxy_url);

    match result {
        Ok((host, port)) => {
            // Try to connect to the proxy host
            let start = std::time::Instant::now();
            match std::net::TcpStream::connect_timeout(
                &format!("{}:{}", host, port)
                    .parse()
                    .unwrap_or_else(|_| std::net::SocketAddr::from(([127, 0, 0, 1], port))),
                Duration::from_secs(5),
            ) {
                Ok(_) => ProxyTestResult {
                    proxy_url: proxy_url.to_string(),
                    success: true,
                    status_code: None,
                    error: None,
                    response_time_ms: Some(start.elapsed().as_millis() as u64),
                },
                Err(e) => ProxyTestResult {
                    proxy_url: proxy_url.to_string(),
                    success: false,
                    status_code: None,
                    error: Some(format!("Connection failed: {}", e)),
                    response_time_ms: None,
                },
            }
        }
        Err(e) => ProxyTestResult {
            proxy_url: proxy_url.to_string(),
            success: false,
            status_code: None,
            error: Some(e),
            response_time_ms: None,
        },
    }
}

/// Parse a proxy URL to extract host and port
fn parse_proxy_url(url: &str) -> Result<(String, u16), String> {
    // Remove protocol prefix
    let without_proto = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .or_else(|| url.strip_prefix("socks5://"))
        .or_else(|| url.strip_prefix("socks://"))
        .unwrap_or(url);

    // Remove credentials if present
    let without_creds = if let Some(at_pos) = without_proto.find('@') {
        &without_proto[at_pos + 1..]
    } else {
        without_proto
    };

    // Remove path if present
    let host_port = without_creds.split('/').next().unwrap_or(without_creds);

    // Split host and port
    if let Some(colon_pos) = host_port.rfind(':') {
        let host = &host_port[..colon_pos];
        let port_str = &host_port[colon_pos + 1..];
        let port = port_str
            .parse::<u16>()
            .map_err(|_| format!("Invalid port: {}", port_str))?;
        Ok((host.to_string(), port))
    } else {
        // Default ports
        if url.starts_with("https://") {
            Ok((host_port.to_string(), 443))
        } else if url.starts_with("socks") {
            Ok((host_port.to_string(), 1080))
        } else {
            Ok((host_port.to_string(), 8080))
        }
    }
}

/// Validate TLS/CA bundle configuration
pub fn validate_tls_config(tls: &TlsConfig) -> Result<(), String> {
    if let Some(ref ca_bundle) = tls.ca_bundle {
        let path = std::path::Path::new(ca_bundle);

        if !path.exists() {
            return Err(format!("CA bundle file not found: {}", ca_bundle));
        }

        if !path.is_file() {
            return Err(format!("CA bundle path is not a file: {}", ca_bundle));
        }

        // Try to read the file to verify permissions
        match std::fs::read(path) {
            Ok(contents) => {
                // Basic validation - check if it looks like a PEM file
                let content_str = String::from_utf8_lossy(&contents);
                if !content_str.contains("-----BEGIN") {
                    return Err(format!(
                        "CA bundle file doesn't appear to be a valid PEM certificate: {}",
                        ca_bundle
                    ));
                }
            }
            Err(e) => {
                return Err(format!("Cannot read CA bundle file {}: {}", ca_bundle, e));
            }
        }
    }

    Ok(())
}

/// Format test results for display
pub fn format_test_results(results: &[ProxyTestResult]) -> String {
    let mut output = String::new();

    for result in results {
        let status = if result.success { "✓" } else { "✗" };
        let time = result
            .response_time_ms
            .map(|ms| format!(" ({}ms)", ms))
            .unwrap_or_default();

        output.push_str(&format!("{} {}{}\n", status, result.proxy_url, time));

        if let Some(ref error) = result.error {
            output.push_str(&format!("  Error: {}\n", error));
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_proxy_url_simple() {
        let (host, port) = parse_proxy_url("http://proxy.corp.com:8080").unwrap();
        assert_eq!(host, "proxy.corp.com");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_parse_proxy_url_with_creds() {
        let (host, port) = parse_proxy_url("http://user:pass@proxy.corp.com:8080").unwrap();
        assert_eq!(host, "proxy.corp.com");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_parse_proxy_url_default_port() {
        let (host, port) = parse_proxy_url("http://proxy.corp.com").unwrap();
        assert_eq!(host, "proxy.corp.com");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_parse_proxy_url_socks() {
        let (host, port) = parse_proxy_url("socks5://proxy.corp.com").unwrap();
        assert_eq!(host, "proxy.corp.com");
        assert_eq!(port, 1080);
    }

    #[test]
    fn test_format_test_results() {
        let results = vec![
            ProxyTestResult {
                proxy_url: "http://proxy:8080".to_string(),
                success: true,
                status_code: None,
                error: None,
                response_time_ms: Some(50),
            },
            ProxyTestResult {
                proxy_url: "https://proxy:8443".to_string(),
                success: false,
                status_code: None,
                error: Some("Connection refused".to_string()),
                response_time_ms: None,
            },
        ];

        let output = format_test_results(&results);
        assert!(output.contains("✓ http://proxy:8080"));
        assert!(output.contains("(50ms)"));
        assert!(output.contains("✗ https://proxy:8443"));
        assert!(output.contains("Connection refused"));
    }
}
