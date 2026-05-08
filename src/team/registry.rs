//! Team config registry for managing shared configuration sources
//!
//! Stores team config sources in ~/.jarvy/team-sources.toml

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// A team configuration source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    /// Human-readable name for this source
    pub name: String,
    /// Base URL for the config repository
    pub url: String,
    /// Description of this source
    #[serde(default)]
    pub description: Option<String>,
    /// Unix timestamp of last sync
    #[serde(default)]
    pub last_sync: Option<u64>,
    /// Available configs discovered from index.toml
    #[serde(default)]
    pub configs: Vec<ConfigEntry>,
}

/// An entry in a team's config index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigEntry {
    /// Config name/identifier
    pub name: String,
    /// Path relative to source URL
    pub path: String,
    /// Description of this config
    #[serde(default)]
    pub description: Option<String>,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Team config source registry
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Registry {
    /// Registered sources
    #[serde(default)]
    pub sources: HashMap<String, Source>,
}

impl Registry {
    /// Load registry from default location
    pub fn load() -> Self {
        let path = Self::registry_path();
        Self::load_from(&path)
    }

    /// Load registry from specific path
    pub fn load_from(path: &PathBuf) -> Self {
        if !path.exists() {
            return Self::default();
        }

        match fs::read_to_string(path) {
            Ok(content) => toml::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save registry to default location
    pub fn save(&self) -> Result<(), RegistryError> {
        let path = Self::registry_path();
        self.save_to(&path)
    }

    /// Save registry to specific path
    pub fn save_to(&self, path: &PathBuf) -> Result<(), RegistryError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| RegistryError::IoError {
                path: parent.display().to_string(),
                error: e.to_string(),
            })?;
        }

        let content = toml::to_string_pretty(self).map_err(|e| RegistryError::SerializeError {
            error: e.to_string(),
        })?;

        fs::write(path, content).map_err(|e| RegistryError::IoError {
            path: path.display().to_string(),
            error: e.to_string(),
        })?;

        Ok(())
    }

    /// Get the default registry path
    fn registry_path() -> PathBuf {
        dirs::home_dir()
            .map(|h| h.join(".jarvy").join("team-sources.toml"))
            .unwrap_or_else(|| PathBuf::from(".jarvy/team-sources.toml"))
    }

    /// Add a new source
    pub fn add_source(
        &mut self,
        name: &str,
        url: &str,
        description: Option<&str>,
    ) -> Result<(), RegistryError> {
        if self.sources.contains_key(name) {
            return Err(RegistryError::SourceExists {
                name: name.to_string(),
            });
        }

        let source = Source {
            name: name.to_string(),
            url: url.to_string(),
            description: description.map(String::from),
            last_sync: None,
            configs: Vec::new(),
        };

        self.sources.insert(name.to_string(), source);
        Ok(())
    }

    /// Remove a source
    pub fn remove_source(&mut self, name: &str) -> Result<Source, RegistryError> {
        self.sources
            .remove(name)
            .ok_or_else(|| RegistryError::SourceNotFound {
                name: name.to_string(),
            })
    }

    /// Get a source by name
    pub fn get_source(&self, name: &str) -> Option<&Source> {
        self.sources.get(name)
    }

    /// Get a mutable source by name
    pub fn get_source_mut(&mut self, name: &str) -> Option<&mut Source> {
        self.sources.get_mut(name)
    }

    /// List all sources
    pub fn list_sources(&self) -> Vec<&Source> {
        self.sources.values().collect()
    }

    /// Sync a source's index
    pub fn sync_source(&mut self, name: &str) -> Result<usize, RegistryError> {
        let source = self
            .sources
            .get_mut(name)
            .ok_or_else(|| RegistryError::SourceNotFound {
                name: name.to_string(),
            })?;

        // Fetch index.toml from source URL
        let index_url = format!("{}/index.toml", source.url.trim_end_matches('/'));

        let response = crate::net::agent()
            .get(&index_url)
            .header("User-Agent", &crate::net::user_agent())
            .call()
            .map_err(|e| RegistryError::FetchError {
                url: index_url.clone(),
                error: e.to_string(),
            })?;

        let body =
            response
                .into_body()
                .read_to_string()
                .map_err(|e| RegistryError::FetchError {
                    url: index_url,
                    error: e.to_string(),
                })?;

        // Parse index
        let index: IndexFile = toml::from_str(&body).map_err(|e| RegistryError::ParseError {
            source: name.to_string(),
            error: e.to_string(),
        })?;

        // Update source
        source.configs = index.configs;
        source.last_sync = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        );

        Ok(source.configs.len())
    }

    /// Find a config by source/name syntax
    pub fn find_config(&self, spec: &str) -> Option<(&Source, &ConfigEntry)> {
        let parts: Vec<&str> = spec.splitn(2, '/').collect();
        if parts.len() != 2 {
            return None;
        }

        let source_name = parts[0];
        let config_name = parts[1];

        let source = self.sources.get(source_name)?;
        let config = source.configs.iter().find(|c| c.name == config_name)?;

        Some((source, config))
    }

    /// Get the full URL for a config
    pub fn get_config_url(&self, spec: &str) -> Option<String> {
        let (source, config) = self.find_config(spec)?;
        Some(format!(
            "{}/{}",
            source.url.trim_end_matches('/'),
            config.path.trim_start_matches('/')
        ))
    }
}

/// Index file format for team config discovery
#[derive(Debug, Deserialize)]
struct IndexFile {
    /// List of available configs
    #[serde(default)]
    configs: Vec<ConfigEntry>,
}

/// Registry errors
#[derive(Debug)]
pub enum RegistryError {
    /// Source already exists
    SourceExists { name: String },
    /// Source not found
    SourceNotFound { name: String },
    /// I/O error
    IoError { path: String, error: String },
    /// Serialization error
    SerializeError { error: String },
    /// Fetch error
    FetchError { url: String, error: String },
    /// Parse error
    ParseError { source: String, error: String },
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::SourceExists { name } => {
                write!(f, "Source '{}' already exists", name)
            }
            RegistryError::SourceNotFound { name } => {
                write!(f, "Source '{}' not found", name)
            }
            RegistryError::IoError { path, error } => {
                write!(f, "I/O error at '{}': {}", path, error)
            }
            RegistryError::SerializeError { error } => {
                write!(f, "Serialization error: {}", error)
            }
            RegistryError::FetchError { url, error } => {
                write!(f, "Failed to fetch '{}': {}", url, error)
            }
            RegistryError::ParseError { source, error } => {
                write!(f, "Failed to parse index from '{}': {}", source, error)
            }
        }
    }
}

impl std::error::Error for RegistryError {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_registry_add_source() {
        let mut registry = Registry::default();

        registry
            .add_source(
                "company",
                "https://example.com/configs",
                Some("Company configs"),
            )
            .unwrap();

        assert_eq!(registry.sources.len(), 1);
        let source = registry.get_source("company").unwrap();
        assert_eq!(source.url, "https://example.com/configs");
    }

    #[test]
    fn test_registry_add_duplicate() {
        let mut registry = Registry::default();

        registry
            .add_source("company", "https://example.com/configs", None)
            .unwrap();

        let result = registry.add_source("company", "https://other.com", None);
        assert!(matches!(result, Err(RegistryError::SourceExists { .. })));
    }

    #[test]
    fn test_registry_remove_source() {
        let mut registry = Registry::default();

        registry
            .add_source("company", "https://example.com/configs", None)
            .unwrap();

        let removed = registry.remove_source("company").unwrap();
        assert_eq!(removed.name, "company");
        assert!(registry.sources.is_empty());
    }

    #[test]
    fn test_registry_remove_nonexistent() {
        let mut registry = Registry::default();
        let result = registry.remove_source("nonexistent");
        assert!(matches!(result, Err(RegistryError::SourceNotFound { .. })));
    }

    #[test]
    fn test_registry_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test-sources.toml");

        let mut registry = Registry::default();
        registry
            .add_source("test", "https://test.com", Some("Test source"))
            .unwrap();

        registry.save_to(&path).unwrap();
        assert!(path.exists());

        let loaded = Registry::load_from(&path);
        assert_eq!(loaded.sources.len(), 1);
        assert!(loaded.sources.contains_key("test"));
    }

    #[test]
    fn test_find_config() {
        let mut registry = Registry::default();
        registry
            .add_source("company", "https://example.com", None)
            .unwrap();

        // Add a config entry
        if let Some(source) = registry.get_source_mut("company") {
            source.configs.push(ConfigEntry {
                name: "frontend".to_string(),
                path: "configs/frontend.toml".to_string(),
                description: Some("Frontend config".to_string()),
                tags: vec!["web".to_string()],
            });
        }

        let result = registry.find_config("company/frontend");
        assert!(result.is_some());

        let (source, config) = result.unwrap();
        assert_eq!(source.name, "company");
        assert_eq!(config.name, "frontend");
    }

    #[test]
    fn test_get_config_url() {
        let mut registry = Registry::default();
        registry
            .add_source("company", "https://example.com/repo", None)
            .unwrap();

        if let Some(source) = registry.get_source_mut("company") {
            source.configs.push(ConfigEntry {
                name: "frontend".to_string(),
                path: "configs/frontend.toml".to_string(),
                description: None,
                tags: vec![],
            });
        }

        let url = registry.get_config_url("company/frontend");
        assert_eq!(
            url,
            Some("https://example.com/repo/configs/frontend.toml".to_string())
        );
    }

    #[test]
    fn test_list_sources() {
        let mut registry = Registry::default();
        registry.add_source("a", "https://a.com", None).unwrap();
        registry.add_source("b", "https://b.com", None).unwrap();

        let sources = registry.list_sources();
        assert_eq!(sources.len(), 2);
    }
}
