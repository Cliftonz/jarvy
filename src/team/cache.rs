//! Configuration caching for remote configs
//!
//! Caches remote configurations locally with configurable TTL.
//! Cache location: ~/.jarvy/cache/configs/

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// Default cache TTL: 1 hour
const DEFAULT_TTL_SECS: u64 = 3600;

/// Configuration cache for remote configs
pub struct ConfigCache {
    /// Directory for cached configs
    cache_dir: PathBuf,
    /// Time-to-live for cached entries
    ttl: Duration,
}

impl ConfigCache {
    /// Create a new config cache with default settings
    pub fn new() -> Self {
        let cache_dir = crate::paths::remote_config_cache_dir()
            .unwrap_or_else(|_| PathBuf::from(".jarvy/cache/configs"));

        Self {
            cache_dir,
            ttl: Duration::from_secs(DEFAULT_TTL_SECS),
        }
    }

    /// Create a config cache with custom TTL
    pub fn with_ttl(mut self, ttl_secs: u64) -> Self {
        self.ttl = Duration::from_secs(ttl_secs);
        self
    }

    /// Create a config cache with custom directory
    pub fn with_dir(mut self, dir: PathBuf) -> Self {
        self.cache_dir = dir;
        self
    }

    /// Get the cache directory path
    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    /// Compute cache key from URL (SHA256-like hash using simple algorithm)
    fn cache_key(&self, url: &str) -> String {
        // Simple hash for cache key - not cryptographic, just for uniqueness
        let hash = url
            .bytes()
            .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
        format!("{:016x}.toml", hash)
    }

    /// Get the cache path for a URL
    pub fn cache_path(&self, url: &str) -> PathBuf {
        self.cache_dir.join(self.cache_key(url))
    }

    /// Get cached config if valid (not expired)
    pub fn get(&self, url: &str) -> Option<String> {
        let path = self.cache_path(url);

        if !path.exists() {
            return None;
        }

        // Check if cache entry is still valid
        if let Ok(metadata) = fs::metadata(&path)
            && let Ok(modified) = metadata.modified()
        {
            let age = SystemTime::now()
                .duration_since(modified)
                .unwrap_or(Duration::MAX);

            if age > self.ttl {
                // Cache expired
                return None;
            }
        }

        fs::read_to_string(&path).ok()
    }

    /// Get cached config even if expired (for offline fallback)
    pub fn get_stale(&self, url: &str) -> Option<String> {
        let path = self.cache_path(url);
        fs::read_to_string(&path).ok()
    }

    /// Check if a URL has a valid (non-expired) cache entry
    pub fn is_valid(&self, url: &str) -> bool {
        self.get(url).is_some()
    }

    /// Check if a URL has any cache entry (even expired)
    pub fn has_entry(&self, url: &str) -> bool {
        self.cache_path(url).exists()
    }

    /// Store config in cache after validating it's valid TOML
    pub fn set(&self, url: &str, content: &str) -> Result<(), CacheError> {
        // Validate TOML before caching
        let _: toml::Value = toml::from_str(content).map_err(|e| CacheError::InvalidToml {
            url: url.to_string(),
            error: e.to_string(),
        })?;

        // Ensure cache directory exists
        if let Err(e) = fs::create_dir_all(&self.cache_dir) {
            return Err(CacheError::IoError {
                path: self.cache_dir.display().to_string(),
                error: e.to_string(),
            });
        }

        let path = self.cache_path(url);
        fs::write(&path, content).map_err(|e| CacheError::IoError {
            path: path.display().to_string(),
            error: e.to_string(),
        })?;

        Ok(())
    }

    /// Remove a cache entry
    pub fn remove(&self, url: &str) -> Result<(), CacheError> {
        let path = self.cache_path(url);
        if path.exists() {
            fs::remove_file(&path).map_err(|e| CacheError::IoError {
                path: path.display().to_string(),
                error: e.to_string(),
            })?;
        }
        Ok(())
    }

    /// Clear all cached configs
    pub fn clear(&self) -> Result<usize, CacheError> {
        if !self.cache_dir.exists() {
            return Ok(0);
        }

        let mut count = 0;
        for entry in fs::read_dir(&self.cache_dir)
            .map_err(|e| CacheError::IoError {
                path: self.cache_dir.display().to_string(),
                error: e.to_string(),
            })?
            .flatten()
        {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "toml") && fs::remove_file(&path).is_ok() {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let mut stats = CacheStats::default();

        if !self.cache_dir.exists() {
            return stats;
        }

        if let Ok(entries) = fs::read_dir(&self.cache_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "toml") {
                    stats.total_entries += 1;

                    if let Ok(metadata) = fs::metadata(&path) {
                        stats.total_size += metadata.len();

                        if let Ok(modified) = metadata.modified() {
                            let age = SystemTime::now()
                                .duration_since(modified)
                                .unwrap_or(Duration::MAX);

                            if age <= self.ttl {
                                stats.valid_entries += 1;
                            }
                        }
                    }
                }
            }
        }

        stats
    }
}

impl Default for ConfigCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics
#[derive(Debug, Default)]
pub struct CacheStats {
    /// Total number of cache entries
    pub total_entries: usize,
    /// Number of valid (non-expired) entries
    pub valid_entries: usize,
    /// Total size in bytes
    pub total_size: u64,
}

/// Cache errors
#[derive(Debug)]
pub enum CacheError {
    /// Invalid TOML content
    InvalidToml { url: String, error: String },
    /// I/O error
    IoError { path: String, error: String },
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheError::InvalidToml { url, error } => {
                write!(f, "Invalid TOML from '{}': {}", url, error)
            }
            CacheError::IoError { path, error } => {
                write!(f, "I/O error at '{}': {}", path, error)
            }
        }
    }
}

impl std::error::Error for CacheError {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_cache() -> (ConfigCache, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let cache = ConfigCache::new().with_dir(temp_dir.path().to_path_buf());
        (cache, temp_dir)
    }

    #[test]
    fn test_cache_key_deterministic() {
        let cache = ConfigCache::new();
        let key1 = cache.cache_key("https://example.com/config.toml");
        let key2 = cache.cache_key("https://example.com/config.toml");
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_cache_key_different_urls() {
        let cache = ConfigCache::new();
        let key1 = cache.cache_key("https://example.com/config.toml");
        let key2 = cache.cache_key("https://other.com/config.toml");
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_cache_set_get() {
        let (cache, _temp) = test_cache();
        let url = "https://example.com/test.toml";
        let content = r#"
[provisioner]
git = "latest"
"#;

        cache.set(url, content).expect("should cache valid TOML");
        let cached = cache.get(url).expect("should get cached content");
        assert_eq!(cached.trim(), content.trim());
    }

    #[test]
    fn test_cache_invalid_toml() {
        let (cache, _temp) = test_cache();
        let url = "https://example.com/invalid.toml";
        let content = "this is not valid TOML {{{";

        let result = cache.set(url, content);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CacheError::InvalidToml { .. }
        ));
    }

    #[test]
    fn test_cache_get_nonexistent() {
        let (cache, _temp) = test_cache();
        let result = cache.get("https://nonexistent.com/config.toml");
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_remove() {
        let (cache, _temp) = test_cache();
        let url = "https://example.com/remove.toml";
        let content = "[provisioner]\ngit = \"latest\"";

        cache.set(url, content).unwrap();
        assert!(cache.has_entry(url));

        cache.remove(url).unwrap();
        assert!(!cache.has_entry(url));
    }

    #[test]
    fn test_cache_clear() {
        let (cache, _temp) = test_cache();

        // Add multiple entries
        for i in 0..5 {
            let url = format!("https://example.com/config{}.toml", i);
            let content = format!("[provisioner]\nvar{} = \"value\"", i);
            cache.set(&url, &content).unwrap();
        }

        let cleared = cache.clear().unwrap();
        assert_eq!(cleared, 5);

        let stats = cache.stats();
        assert_eq!(stats.total_entries, 0);
    }

    #[test]
    fn test_cache_stats() {
        let (cache, _temp) = test_cache();

        // Add entries
        for i in 0..3 {
            let url = format!("https://example.com/stats{}.toml", i);
            let content = format!("[provisioner]\nval{} = \"x\"", i);
            cache.set(&url, &content).unwrap();
        }

        let stats = cache.stats();
        assert_eq!(stats.total_entries, 3);
        assert_eq!(stats.valid_entries, 3);
        assert!(stats.total_size > 0);
    }

    #[test]
    fn test_cache_ttl() {
        let (cache, _temp) = test_cache();
        // Use very short TTL for testing
        let cache = cache.with_ttl(0);

        let url = "https://example.com/ttl.toml";
        let content = "[provisioner]\ngit = \"latest\"";

        cache.set(url, content).unwrap();

        // With 0 TTL, cache should be immediately invalid
        assert!(cache.get(url).is_none());

        // But stale should still work
        assert!(cache.get_stale(url).is_some());
    }
}
