//! Remote configuration fetching utilities
//!
//! This module handles fetching jarvy.toml configurations from remote URLs
//! with caching support for GitHub raw URLs, gists, and other HTTP endpoints.

use std::fs;
use std::io::{Read, Write};
use std::time::Duration;

/// Maximum size for remote config files (1MB)
pub const MAX_REMOTE_CONFIG_SIZE: u64 = 1024 * 1024;

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
/// - Use --insecure to skip SSL verification (not recommended)
///
/// Security:
/// - Enforces 1MB size limit to prevent memory exhaustion
pub fn fetch_remote_config(
    url: &str,
    _insecure: bool,
    headers: &[String],
) -> Result<String, String> {
    // Get cache directory
    let cache_dir = dirs::home_dir()
        .ok_or("Could not determine home directory")?
        .join(".jarvy")
        .join("cache")
        .join("configs");

    // Create cache directory if it doesn't exist
    if !cache_dir.exists() {
        fs::create_dir_all(&cache_dir)
            .map_err(|e| format!("Failed to create cache directory: {}", e))?;
    }

    // Generate cache key from URL (simple hash)
    let cache_key = url
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
    let cache_file = cache_dir.join(format!("{:x}.toml", cache_key));
    let cache_meta = cache_dir.join(format!("{:x}.meta", cache_key));

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
        return Ok(cache_file.to_string_lossy().to_string());
    }

    println!("Fetching config from {}...", url);

    // Transform GitHub URLs to raw URLs if needed
    let fetch_url = transform_github_url(url);

    // Create HTTP agent
    let agent = ureq::Agent::new_with_defaults();

    // Build the request with default headers
    let mut request = agent
        .get(&fetch_url)
        .header(
            "User-Agent",
            "Jarvy/0.1 (https://github.com/bearbinary/jarvy)",
        )
        .header("Accept", "text/plain, application/toml, */*");

    // Add custom headers (for authentication, etc.)
    for header in headers {
        if let Some((key, value)) = header.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            if !key.is_empty() && !value.is_empty() {
                request = request.header(key, value);
            } else {
                eprintln!(
                    "Warning: Invalid header format '{}', expected 'Name: Value'",
                    header
                );
            }
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
    if let Some(content_length) = response.headers().get("content-length") {
        if let Some(length) = content_length
            .to_str()
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
        {
            if length > MAX_REMOTE_CONFIG_SIZE {
                return Err(format!(
                    "Remote config too large: {} bytes (max {} bytes)",
                    length, MAX_REMOTE_CONFIG_SIZE
                ));
            }
        }
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
        return Err(format!(
            "Remote config too large: exceeds {} bytes limit",
            MAX_REMOTE_CONFIG_SIZE
        ));
    }

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
