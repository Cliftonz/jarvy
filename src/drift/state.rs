//! Environment state management for drift detection

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use super::DriftError;

/// Captured environment state
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EnvironmentState {
    /// State file format version
    pub version: String,

    /// When state was first captured
    pub created_at: String,

    /// When state was last updated
    pub updated_at: String,

    /// Hash of the jarvy.toml config file
    pub config_hash: String,

    /// Tool states keyed by tool name
    pub tools: HashMap<String, ToolState>,

    /// File hashes keyed by relative path
    pub files: HashMap<String, String>,
}

/// State of a single installed tool
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolState {
    /// Installed version
    pub version: String,

    /// Path to the tool binary
    pub path: PathBuf,

    /// How the tool was installed (brew, apt, rustup, etc.)
    pub install_method: String,
}

impl Default for EnvironmentState {
    fn default() -> Self {
        Self {
            version: "1".to_string(),
            created_at: current_timestamp(),
            updated_at: current_timestamp(),
            config_hash: String::new(),
            tools: HashMap::new(),
            files: HashMap::new(),
        }
    }
}

impl EnvironmentState {
    /// Create a new empty state
    pub fn new() -> Self {
        Self::default()
    }

    /// Load state from the project's .jarvy/state.json file
    pub fn load(project_dir: &Path) -> Result<Option<Self>, DriftError> {
        let state_path = project_dir.join(".jarvy/state.json");

        if !state_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&state_path)?;
        let state: Self = serde_json::from_str(&content)?;
        Ok(Some(state))
    }

    /// Save state to the project's .jarvy/state.json file
    pub fn save(&self, project_dir: &Path) -> Result<(), DriftError> {
        let jarvy_dir = project_dir.join(".jarvy");
        fs::create_dir_all(&jarvy_dir)?;

        let state_path = jarvy_dir.join("state.json");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&state_path, content)?;
        Ok(())
    }

    /// Add or update a tool in the state
    pub fn set_tool(&mut self, name: &str, version: &str, path: &Path, install_method: &str) {
        self.tools.insert(
            name.to_string(),
            ToolState {
                version: version.to_string(),
                path: path.to_path_buf(),
                install_method: install_method.to_string(),
            },
        );
        self.updated_at = current_timestamp();
    }

    /// Remove a tool from the state
    #[allow(dead_code)]
    pub fn remove_tool(&mut self, name: &str) {
        self.tools.remove(name);
        self.updated_at = current_timestamp();
    }

    /// Add or update a tracked file hash
    pub fn set_file_hash(&mut self, path: &str, hash: &str) {
        self.files.insert(path.to_string(), hash.to_string());
        self.updated_at = current_timestamp();
    }

    /// Set the config file hash
    pub fn set_config_hash(&mut self, hash: &str) {
        self.config_hash = hash.to_string();
        self.updated_at = current_timestamp();
    }

    /// Get the number of tracked tools
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Get the number of tracked files
    #[allow(dead_code)]
    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}

/// Hash a file using SHA-256
pub fn hash_file(path: &Path) -> Result<String, DriftError> {
    let mut file = fs::File::open(path)
        .map_err(|e| DriftError::HashError(format!("{}: {}", path.display(), e)))?;

    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file
            .read(&mut buffer)
            .map_err(|e| DriftError::HashError(format!("{}: {}", path.display(), e)))?;

        if bytes_read == 0 {
            break;
        }

        hasher.update(&buffer[..bytes_read]);
    }

    let result = hasher.finalize();
    Ok(format!("sha256:{}", hex::encode(result)))
}

/// Hash a string using SHA-256
#[allow(dead_code)]
pub fn hash_string(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let result = hasher.finalize();
    format!("sha256:{}", hex::encode(result))
}

/// Get current timestamp in ISO 8601 format
fn current_timestamp() -> String {
    // Use a simple format without chrono dependency
    let now = std::time::SystemTime::now();
    let duration = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();

    // Simple UTC timestamp (approximate)
    format!("{}Z", secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_environment_state_new() {
        let state = EnvironmentState::new();
        assert_eq!(state.version, "1");
        assert!(state.tools.is_empty());
        assert!(state.files.is_empty());
    }

    #[test]
    fn test_set_tool() {
        let mut state = EnvironmentState::new();
        state.set_tool("node", "20.10.0", Path::new("/usr/bin/node"), "brew");

        assert_eq!(state.tools.len(), 1);
        let tool = state.tools.get("node").unwrap();
        assert_eq!(tool.version, "20.10.0");
        assert_eq!(tool.install_method, "brew");
    }

    #[test]
    fn test_remove_tool() {
        let mut state = EnvironmentState::new();
        state.set_tool("node", "20.10.0", Path::new("/usr/bin/node"), "brew");
        state.remove_tool("node");

        assert!(state.tools.is_empty());
    }

    #[test]
    fn test_set_file_hash() {
        let mut state = EnvironmentState::new();
        state.set_file_hash(".vscode/settings.json", "sha256:abc123");

        assert_eq!(state.files.len(), 1);
        assert_eq!(
            state.files.get(".vscode/settings.json"),
            Some(&"sha256:abc123".to_string())
        );
    }

    #[test]
    fn test_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        let mut state = EnvironmentState::new();
        state.set_tool("node", "20.10.0", Path::new("/usr/bin/node"), "brew");
        state.set_file_hash("package.json", "sha256:def456");

        // Save
        state.save(project_dir).unwrap();

        // Verify file exists
        let state_path = project_dir.join(".jarvy/state.json");
        assert!(state_path.exists());

        // Load
        let loaded = EnvironmentState::load(project_dir).unwrap().unwrap();
        assert_eq!(loaded.tools.len(), 1);
        assert_eq!(loaded.files.len(), 1);
    }

    #[test]
    fn test_load_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let result = EnvironmentState::load(temp_dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_hash_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(b"hello world").unwrap();

        let hash = hash_file(&file_path).unwrap();
        assert!(hash.starts_with("sha256:"));
        // SHA-256 of "hello world" is known
        assert!(hash.contains("b94d27b9934d3e08a52e52d7da7dabfa"));
    }

    #[test]
    fn test_hash_string() {
        let hash = hash_string("hello world");
        assert!(hash.starts_with("sha256:"));
        assert!(hash.contains("b94d27b9934d3e08a52e52d7da7dabfa"));
    }
}
