//! Remote configuration fetching utilities
//!
//! This module handles fetching jarvy.toml configurations from remote URLs
//! with caching support for GitHub raw URLs, gists, and other HTTP endpoints.

use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::time::Duration;

/// Maximum size for remote config files (1MB)
pub const MAX_REMOTE_CONFIG_SIZE: u64 = 1024 * 1024;

/// Hosts permitted as remote config sources by default. The intent is to
/// require an explicit operator override (`JARVY_ALLOW_REMOTE_HOST`) before
/// `--from <url>` will fetch from anywhere else, so a typo or pasted attacker
/// link cannot exfiltrate `Authorization` headers or seed a malicious
/// `jarvy.toml`.
const DEFAULT_ALLOWED_HOSTS: &[&str] = &[
    "github.com",
    "raw.githubusercontent.com",
    "gist.github.com",
    "gist.githubusercontent.com",
    "gitlab.com",
    "bitbucket.org",
];

/// Header names that must NEVER cross an origin boundary. Matches
/// case-insensitively. Anything in this list is dropped from the user's
/// custom-header set when the URL host is outside the allowlist or when the
/// agent has to follow a redirect to a different origin.
const SENSITIVE_HEADER_NAMES: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "cookie",
    "x-api-key",
];

#[derive(Debug)]
struct UrlPolicy<'a> {
    scheme: &'a str,
    host: String,
}

fn parse_url_policy(url: &str) -> Result<UrlPolicy<'_>, String> {
    let Some(scheme_end) = url.find("://") else {
        return Err(format!("URL is missing a scheme: {url}"));
    };
    let scheme = &url[..scheme_end];
    let after = &url[scheme_end + 3..];
    let host_end = after.find(['/', '?', '#']).unwrap_or(after.len());
    let authority = &after[..host_end];
    // Strip optional userinfo before the '@'.
    let host_with_port = match authority.find('@') {
        Some(at) => &authority[at + 1..],
        None => authority,
    };
    // Strip port suffix.
    let host = match host_with_port.find(':') {
        Some(p) => &host_with_port[..p],
        None => host_with_port,
    };
    if host.is_empty() {
        return Err(format!("URL has empty host: {url}"));
    }
    Ok(UrlPolicy {
        scheme,
        host: host.to_ascii_lowercase(),
    })
}

fn host_in_allowlist(host: &str, allow_extra: &[String]) -> bool {
    if DEFAULT_ALLOWED_HOSTS.contains(&host) {
        return true;
    }
    if allow_extra.iter().any(|h| h.eq_ignore_ascii_case(host)) {
        return true;
    }
    // Permit subdomains of allowlisted hosts (e.g., *.github.com).
    DEFAULT_ALLOWED_HOSTS
        .iter()
        .any(|allowed| host.ends_with(&format!(".{allowed}")))
}

fn extra_allowed_hosts_from_env() -> Vec<String> {
    std::env::var("JARVY_ALLOW_REMOTE_HOST")
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn header_is_sensitive(name: &str) -> bool {
    SENSITIVE_HEADER_NAMES
        .iter()
        .any(|h| name.eq_ignore_ascii_case(h))
}

/// Fetch a jarvy.toml configuration from a remote URL with caching
///
/// PRD-015/016: Remote config loading support
///
/// Supports:
/// - GitHub raw URLs
/// - Gist URLs
/// - Any HTTP/HTTPS URL returning TOML content
/// - Custom headers for authenticated requests
///
/// Caching:
/// - Configs are cached in ~/.jarvy/cache/configs/
/// - Cache expires after 1 hour
///
/// Security:
/// - Always verifies TLS certificates (no insecure bypass)
/// - Enforces 1MB size limit to prevent memory exhaustion
/// - Rejects URLs whose scheme is not `https` (or `http://localhost` for tests)
/// - Rejects hosts outside the default allowlist unless the operator explicitly
///   sets `JARVY_ALLOW_REMOTE_HOST="host1,host2"`
/// - Strips sensitive headers (`Authorization`, `Cookie`, etc.) before sending
///   to non-allowlisted hosts so a typo in `--from` cannot exfiltrate tokens
pub fn fetch_remote_config(url: &str, headers: &[String]) -> Result<String, String> {
    // Validate URL scheme + host before doing any IO.
    let policy = parse_url_policy(url)?;
    let allow_extra = extra_allowed_hosts_from_env();
    let scheme_ok = policy.scheme == "https"
        || (policy.scheme == "http" && (policy.host == "localhost" || policy.host == "127.0.0.1"));
    if !scheme_ok {
        return Err(format!(
            "Refusing to fetch remote config over scheme '{}'. Use https://. \
             (URL: {url})",
            policy.scheme
        ));
    }
    let host_allowed = host_in_allowlist(&policy.host, &allow_extra);
    if !host_allowed {
        return Err(format!(
            "Refusing to fetch remote config from host '{}'. \
             Allowed hosts: {}. \
             To permit a custom host, set JARVY_ALLOW_REMOTE_HOST=\"host1,host2\".",
            policy.host,
            DEFAULT_ALLOWED_HOSTS.join(", ")
        ));
    }

    // Get cache directory (canonical resolver: honors JARVY_HOME override).
    let cache_dir = crate::paths::remote_config_cache_dir()
        .map_err(|e| format!("Could not determine home directory: {e}"))?;

    // Create cache directory if it doesn't exist
    if !cache_dir.exists() {
        fs::create_dir_all(&cache_dir)
            .map_err(|e| format!("Failed to create cache directory: {}", e))?;
    }

    // Generate cache key from URL using SHA-256 (collision-resistant)
    let cache_key = hex::encode(Sha256::digest(url.as_bytes()));
    let cache_file = cache_dir.join(format!("{}.toml", &cache_key[..16]));
    let cache_meta = cache_dir.join(format!("{}.meta", &cache_key[..16]));

    // Check if cached file exists and is fresh (< 1 hour old)
    let cache_valid = if cache_file.exists() && cache_meta.exists() {
        if let Ok(metadata) = fs::metadata(&cache_meta) {
            if let Ok(modified) = metadata.modified() {
                modified
                    .elapsed()
                    .map(|d| d < Duration::from_secs(3600))
                    .unwrap_or(false)
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };

    if cache_valid {
        println!("Using cached config from {}", url);
        tracing::debug!(
            event = "remote_config.cache.hit",
            url_hash = &cache_key[..16],
        );
        return Ok(cache_file.to_string_lossy().to_string());
    }

    println!("Fetching config from {}...", url);
    tracing::info!(
        event = "remote_config.fetch.start",
        url_hash = &cache_key[..16],
        host = %policy.host,
    );

    // Transform GitHub URLs to raw URLs if needed
    let fetch_url = transform_github_url(url);

    // Use the process-wide shared agent (timeouts + connection reuse).
    let agent = crate::net::agent();

    // Build the request with default headers
    let mut request = agent
        .get(&fetch_url)
        .header(
            "User-Agent",
            "Jarvy/0.1 (https://github.com/Cliftonz/jarvy)",
        )
        .header("Accept", "text/plain, application/toml, */*");

    // Add custom headers (for authentication, etc.). Sensitive headers are
    // dropped if the host falls outside the default allowlist — even when an
    // operator-supplied `JARVY_ALLOW_REMOTE_HOST` covers the host — to limit
    // the blast radius of a misconfigured allowlist.
    let host_in_default_list = DEFAULT_ALLOWED_HOSTS
        .iter()
        .any(|h| policy.host == *h || policy.host.ends_with(&format!(".{h}")));
    for header in headers {
        if let Some((key, value)) = header.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            if key.is_empty() || value.is_empty() {
                eprintln!(
                    "Warning: Invalid header format '{}', expected 'Name: Value'",
                    header
                );
                continue;
            }
            if header_is_sensitive(key) && !host_in_default_list {
                tracing::warn!(
                    event = "remote_config.header_dropped",
                    header_name = %key,
                    host = %policy.host,
                    reason = "non_default_host"
                );
                continue;
            }
            request = request.header(key, value);
        } else {
            eprintln!(
                "Warning: Invalid header format '{}', expected 'Name: Value'",
                header
            );
        }
    }

    // Fetch the config
    let response = request
        .call()
        .map_err(|e| format!("Failed to fetch config: {}", e))?;

    if response.status() != 200 {
        return Err(format!("HTTP error {}", response.status()));
    }

    // Check content-length header if available
    if let Some(content_length) = response.headers().get("content-length")
        && let Some(length) = content_length
            .to_str()
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
        && length > MAX_REMOTE_CONFIG_SIZE
    {
        return Err(format!(
            "Remote config too large: {} bytes (max {} bytes)",
            length, MAX_REMOTE_CONFIG_SIZE
        ));
    }

    // Read with size limit (even if Content-Length was not present or was incorrect)
    let mut content = String::new();
    let mut body = response.into_body();
    let reader = body.as_reader();
    let mut limited_reader = reader.take(MAX_REMOTE_CONFIG_SIZE + 1);

    limited_reader
        .read_to_string(&mut content)
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    // Check if we hit the limit
    if content.len() as u64 > MAX_REMOTE_CONFIG_SIZE {
        tracing::warn!(
            event = "remote_config.size_limit_exceeded",
            url_hash = &cache_key[..16],
            max = MAX_REMOTE_CONFIG_SIZE,
        );
        return Err(format!(
            "Remote config too large: exceeds {} bytes limit",
            MAX_REMOTE_CONFIG_SIZE
        ));
    }
    tracing::info!(
        event = "remote_config.fetch.complete",
        url_hash = &cache_key[..16],
        bytes = content.len(),
    );

    // Validate that content is valid TOML
    let _: toml::Value =
        toml::from_str(&content).map_err(|e| format!("Invalid TOML in remote config: {}", e))?;

    // Write to cache
    let mut file =
        fs::File::create(&cache_file).map_err(|e| format!("Failed to create cache file: {}", e))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write cache file: {}", e))?;

    // Write metadata (URL for reference)
    let mut meta_file = fs::File::create(&cache_meta)
        .map_err(|e| format!("Failed to create cache metadata: {}", e))?;
    meta_file
        .write_all(url.as_bytes())
        .map_err(|e| format!("Failed to write cache metadata: {}", e))?;

    println!("Config cached at {}", cache_file.display());

    Ok(cache_file.to_string_lossy().to_string())
}

/// Validate a URL against Jarvy's network policy and fetch its body.
///
/// This is the entry point for any subsystem that needs to GET a Jarvy-
/// configurable URL — `team::extends`, `team::registry::index.toml`,
/// future MCP-config sync, etc. It enforces the same rules that
/// `fetch_remote_config` enforces (`https://` only outside loopback,
/// allowlisted host, body size cap, TOML pre-validate is the caller's
/// job) so a hostile `extends = "http://10.0.0.1/admin/internal.toml"`
/// is refused at one place rather than slipping through a second-impl
/// fetch elsewhere in the tree (security review F-3).
///
/// Differences vs `fetch_remote_config`:
/// - No on-disk cache (callers may have their own caching layer).
/// - No GitHub URL rewrite (callers know what they want).
/// - No header injection (no `--from` headers in this path).
pub fn validated_get(url: &str) -> Result<String, String> {
    let policy = parse_url_policy(url)?;
    let allow_extra = extra_allowed_hosts_from_env();
    let scheme_ok = policy.scheme == "https"
        || (policy.scheme == "http" && (policy.host == "localhost" || policy.host == "127.0.0.1"));
    if !scheme_ok {
        return Err(format!(
            "Refusing to fetch over scheme '{}'. Use https://. (URL: {url})",
            policy.scheme
        ));
    }
    if !host_in_allowlist(&policy.host, &allow_extra) {
        return Err(format!(
            "Refusing to fetch from host '{}'. Allowed hosts: {}. \
             To permit a custom host, set JARVY_ALLOW_REMOTE_HOST=\"host1,host2\".",
            policy.host,
            DEFAULT_ALLOWED_HOSTS.join(", ")
        ));
    }

    let response = crate::net::agent()
        .get(url)
        .header("User-Agent", crate::net::USER_AGENT)
        .header("Accept", "text/plain, application/toml, */*")
        .call()
        .map_err(|e| format!("Failed to fetch URL: {}", e))?;

    if response.status() != 200 {
        return Err(format!("HTTP error {}", response.status()));
    }

    if let Some(content_length) = response.headers().get("content-length")
        && let Some(length) = content_length
            .to_str()
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
        && length > MAX_REMOTE_CONFIG_SIZE
    {
        return Err(format!(
            "Remote response too large: {} bytes (max {} bytes)",
            length, MAX_REMOTE_CONFIG_SIZE
        ));
    }

    let mut content = String::new();
    let mut body = response.into_body();
    let reader = body.as_reader();
    let mut limited = reader.take(MAX_REMOTE_CONFIG_SIZE + 1);
    limited
        .read_to_string(&mut content)
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    if content.len() as u64 > MAX_REMOTE_CONFIG_SIZE {
        return Err(format!(
            "Remote response too large: exceeds {} bytes limit",
            MAX_REMOTE_CONFIG_SIZE
        ));
    }

    Ok(content)
}

/// Transform GitHub URLs to raw content URLs
pub fn transform_github_url(url: &str) -> String {
    // Transform github.com blob URLs to raw.githubusercontent.com
    // e.g., https://github.com/user/repo/blob/main/jarvy.toml
    // -> https://raw.githubusercontent.com/user/repo/main/jarvy.toml
    if url.contains("github.com") && url.contains("/blob/") {
        return url
            .replace("github.com", "raw.githubusercontent.com")
            .replace("/blob/", "/");
    }

    // Transform gist URLs to raw
    // e.g., https://gist.github.com/user/hash
    // -> https://gist.githubusercontent.com/user/hash/raw
    if url.contains("gist.github.com") && !url.contains("/raw") {
        return format!("{}/raw", url.trim_end_matches('/'));
    }

    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_url_policy_extracts_host_lowercase() {
        let p = parse_url_policy("https://Raw.GithubUserContent.Com/x/y").unwrap();
        assert_eq!(p.scheme, "https");
        assert_eq!(p.host, "raw.githubusercontent.com");
    }

    #[test]
    fn parse_url_policy_strips_userinfo_and_port() {
        let p = parse_url_policy("https://user:tok@github.com:8443/x").unwrap();
        assert_eq!(p.host, "github.com");
    }

    #[test]
    fn parse_url_policy_rejects_missing_scheme() {
        assert!(parse_url_policy("github.com/owner/repo").is_err());
    }

    #[test]
    fn allowlist_accepts_default_hosts_and_subdomains() {
        assert!(host_in_allowlist("github.com", &[]));
        assert!(host_in_allowlist("raw.githubusercontent.com", &[]));
        assert!(host_in_allowlist("api.github.com", &[])); // subdomain of github.com
        assert!(!host_in_allowlist("attacker.tld", &[]));
    }

    #[test]
    fn allowlist_honors_extra_hosts() {
        let extra = vec!["internal.corp".to_string()];
        assert!(host_in_allowlist("internal.corp", &extra));
        assert!(!host_in_allowlist("attacker.tld", &extra));
    }

    #[test]
    fn fetch_rejects_http_to_remote_host() {
        let err = fetch_remote_config("http://example.com/x.toml", &[])
            .expect_err("http to remote host must be refused");
        assert!(err.contains("scheme") || err.contains("host"));
    }

    #[test]
    fn fetch_rejects_disallowed_host() {
        let err = fetch_remote_config("https://attacker.tld/jarvy.toml", &[])
            .expect_err("disallowed host must be refused");
        assert!(err.contains("attacker.tld"));
    }

    // ----- validated_get rejection tests (round-2 QA B3).
    // The team::* pipeline goes through validated_get (not
    // fetch_remote_config); it has its own copy of the policy gates
    // that can drift independently. These tests pin the rejections.

    #[test]
    fn validated_get_rejects_http_to_remote_host() {
        let err = validated_get("http://example.com/x.toml")
            .expect_err("http to non-loopback must be refused");
        assert!(
            err.contains("scheme") || err.contains("https"),
            "got {err:?}"
        );
    }

    #[test]
    fn validated_get_rejects_disallowed_host() {
        let err = validated_get("https://attacker.tld/internal.toml")
            .expect_err("disallowed host must be refused");
        assert!(err.contains("attacker.tld"), "got {err:?}");
    }

    #[test]
    fn validated_get_rejects_file_scheme() {
        // `file:///etc/passwd` parses with empty host; either rejection
        // path is fine — the point is the URL must NOT be fetched.
        let err = validated_get("file:///etc/passwd").expect_err("file:// must be refused");
        let _ = err;
    }

    #[test]
    fn validated_get_rejects_missing_scheme() {
        let err = validated_get("github.com/owner/repo/blob/main/x.toml")
            .expect_err("missing scheme must be refused");
        let _ = err;
    }

    #[test]
    fn header_sensitivity_check_is_case_insensitive() {
        assert!(header_is_sensitive("Authorization"));
        assert!(header_is_sensitive("authorization"));
        assert!(header_is_sensitive("X-API-Key"));
        assert!(!header_is_sensitive("User-Agent"));
        assert!(!header_is_sensitive("Accept"));
    }

    #[test]
    fn transform_github_url_rewrites_blob_paths() {
        let cases: &[(&str, &str)] = &[
            (
                "https://github.com/u/r/blob/main/jarvy.toml",
                "https://raw.githubusercontent.com/u/r/main/jarvy.toml",
            ),
            (
                // Branch with a slash (e.g. `feat/foo`).
                "https://github.com/u/r/blob/feat/foo/jarvy.toml",
                "https://raw.githubusercontent.com/u/r/feat/foo/jarvy.toml",
            ),
            (
                // Percent-encoded path segment must survive.
                "https://github.com/u/r/blob/main/path%20with%20space.toml",
                "https://raw.githubusercontent.com/u/r/main/path%20with%20space.toml",
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(
                transform_github_url(input),
                *expected,
                "transform_github_url({input:?})"
            );
        }
    }

    #[test]
    fn transform_github_url_appends_raw_to_gists() {
        assert_eq!(
            transform_github_url("https://gist.github.com/u/abc"),
            "https://gist.github.com/u/abc/raw"
        );
        assert_eq!(
            // Trailing slash should be normalized before /raw.
            transform_github_url("https://gist.github.com/u/abc/"),
            "https://gist.github.com/u/abc/raw"
        );
    }

    #[test]
    fn transform_github_url_does_not_double_suffix_raw_gists() {
        // If the URL already has /raw, leave it alone.
        let already_raw = "https://gist.github.com/u/abc/raw";
        assert_eq!(transform_github_url(already_raw), already_raw);
    }

    #[test]
    fn transform_github_url_passes_unrelated_hosts_through() {
        let unrelated = "https://example.com/x.toml";
        assert_eq!(transform_github_url(unrelated), unrelated);
    }
}
