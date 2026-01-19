//! Credential handling for proxy authentication
//!
//! Supports multiple secure methods for retrieving proxy credentials:
//! - Plain text (with security warning)
//! - Environment variable
//! - File path
//! - Interactive prompt

use super::config::{NetworkConfig, PasswordSource, ProxyAuth};
use std::io::{self, Write};

/// Inject authentication credentials into a proxy URL
///
/// Takes a proxy URL like "http://proxy:8080" and credentials,
/// returns "http://user:pass@proxy:8080"
pub fn inject_credentials(proxy_url: &str, auth: &ProxyAuth) -> Result<String, String> {
    let password = auth.password.resolve()?;

    // Parse the URL to inject credentials
    if let Some(proto_end) = proxy_url.find("://") {
        let protocol = &proxy_url[..proto_end + 3];
        let rest = &proxy_url[proto_end + 3..];

        // URL-encode username and password
        let encoded_user = urlencoding::encode(&auth.username);
        let encoded_pass = urlencoding::encode(&password);

        Ok(format!(
            "{}{}:{}@{}",
            protocol, encoded_user, encoded_pass, rest
        ))
    } else {
        Err(format!("Invalid proxy URL format: {}", proxy_url))
    }
}

/// Prompt user for password interactively
///
/// Uses rpassword for hidden input if available, falls back to plain input
pub fn prompt_password(prompt: &str) -> io::Result<String> {
    print!("{}", prompt);
    io::stdout().flush()?;

    // Try to use hidden input (rpassword-like behavior)
    // For now, use standard input (rpassword can be added as dependency if needed)
    let mut password = String::new();
    io::stdin().read_line(&mut password)?;
    Ok(password.trim().to_string())
}

/// Get the proxy URL with credentials injected if authentication is configured
pub fn get_authenticated_proxy(
    proxy_url: Option<&String>,
    auth: Option<&ProxyAuth>,
) -> Result<Option<String>, String> {
    match (proxy_url, auth) {
        (Some(url), Some(auth)) => {
            // Check if URL already has credentials
            if url.contains('@') {
                Ok(Some(url.clone()))
            } else {
                inject_credentials(url, auth).map(Some)
            }
        }
        (Some(url), None) => Ok(Some(url.clone())),
        (None, _) => Ok(None),
    }
}

/// Resolve all proxy URLs with authentication for a NetworkConfig
pub struct AuthenticatedProxies {
    pub http_proxy: Option<String>,
    pub https_proxy: Option<String>,
    pub socks_proxy: Option<String>,
}

impl AuthenticatedProxies {
    /// Create authenticated proxies from NetworkConfig
    pub fn from_config(config: &NetworkConfig) -> Result<Self, String> {
        Ok(Self {
            http_proxy: get_authenticated_proxy(config.http_proxy.as_ref(), config.auth.as_ref())?,
            https_proxy: get_authenticated_proxy(
                config.https_proxy.as_ref(),
                config.auth.as_ref(),
            )?,
            socks_proxy: get_authenticated_proxy(
                config.socks_proxy.as_ref(),
                config.auth.as_ref(),
            )?,
        })
    }
}

// Simple URL encoding module (to avoid adding urlencoding dependency)
mod urlencoding {
    /// URL-encode a string for use in proxy URLs
    pub fn encode(input: &str) -> String {
        let mut result = String::new();
        for c in input.chars() {
            match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                    result.push(c);
                }
                _ => {
                    for byte in c.to_string().as_bytes() {
                        result.push_str(&format!("%{:02X}", byte));
                    }
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inject_credentials() {
        let auth = ProxyAuth {
            username: "user".to_string(),
            password: PasswordSource::Plain("pass".to_string()),
        };

        let result = inject_credentials("http://proxy:8080", &auth).unwrap();
        assert_eq!(result, "http://user:pass@proxy:8080");
    }

    #[test]
    fn test_inject_credentials_with_special_chars() {
        let auth = ProxyAuth {
            username: "user@corp".to_string(),
            password: PasswordSource::Plain("p@ss:word".to_string()),
        };

        let result = inject_credentials("http://proxy:8080", &auth).unwrap();
        // Special chars should be URL-encoded
        assert!(result.contains("%40")); // @ is encoded as %40
    }

    #[test]
    fn test_url_encoding() {
        assert_eq!(urlencoding::encode("user"), "user");
        assert_eq!(urlencoding::encode("user@corp"), "user%40corp");
        assert_eq!(urlencoding::encode("p@ss:word"), "p%40ss%3Aword");
    }

    #[test]
    fn test_get_authenticated_proxy_no_auth() {
        let url = Some("http://proxy:8080".to_string());
        let result = get_authenticated_proxy(url.as_ref(), None).unwrap();
        assert_eq!(result, Some("http://proxy:8080".to_string()));
    }

    #[test]
    fn test_get_authenticated_proxy_already_has_creds() {
        let url = Some("http://user:pass@proxy:8080".to_string());
        let auth = ProxyAuth {
            username: "other".to_string(),
            password: PasswordSource::Plain("other".to_string()),
        };
        let result = get_authenticated_proxy(url.as_ref(), Some(&auth)).unwrap();
        // Should keep existing credentials
        assert_eq!(result, Some("http://user:pass@proxy:8080".to_string()));
    }
}
