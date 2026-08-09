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

/// State of a single installed tool.
///
/// Historically `version` was described as "installed version" but the
/// setup-time write path stored the config constraint (`"latest"`,
/// `"^20"`) here — which caused every drift check to false-positive
/// (Codex P1). The field is now the **config constraint**;
/// `installed_version` (added later via `#[serde(default)]` so legacy
/// state.json files still deserialize) is the concrete version the
/// probe returned at capture time. Detector reads `installed_version`
/// when present, falling back to `version` for pre-migration state
/// files (bounded false-positive window per tool per user).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolState {
    /// Config constraint (`"latest"`, `"^20"`, `"20.10.0"`). Preserved
    /// for parity with the source jarvy.toml and for pre-migration
    /// state-file compatibility.
    pub version: String,

    /// Concrete version returned by the binary probe at capture time
    /// (`"20.10.0"`). Absent on state files written before the P1 fix
    /// or when the probe returned nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_version: Option<String>,

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

    /// Load state from the project's .jarvy/state.json file. Path
    /// resolved through `crate::paths::state_json` so the canonical
    /// project-local resolver is the only site that knows the
    /// `.jarvy/state.json` literal (round-2 maint F3).
    pub fn load(project_dir: &Path) -> Result<Option<Self>, DriftError> {
        let state_path = crate::paths::state_json(project_dir);

        if !state_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&state_path)?;
        let state: Self = serde_json::from_str(&content)?;
        Ok(Some(state))
    }

    /// Save state to the project's .jarvy/state.json file (canonical
    /// resolver in `crate::paths`).
    ///
    /// Write path is tmp+rename+chmod-0600 with a 0700-tightened
    /// parent dir so state.json isn't world-readable on shared build
    /// hosts (its `ToolState.path` field records absolute install
    /// paths like `/Users/<user>/.nvm/…`, leaking the account name
    /// and home layout — security F4). Uses `to_string` not
    /// `to_string_pretty`: state.json is a machine artifact, saving
    /// ~2× disk write per baseline (perf F5).
    pub fn save(&self, project_dir: &Path) -> Result<(), DriftError> {
        let state_path = crate::paths::state_json(project_dir);
        if let Some(parent) = state_path.parent() {
            fs::create_dir_all(parent)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                // Best-effort — some filesystems (NFS, drvfs, exFAT)
                // silently ignore chmod; state.json still lands.
                let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
            }
        }
        let content = serde_json::to_string(self)?;
        // Atomic tmp+rename so a partial write can't leave state.json
        // in a half-parseable shape (concurrent readers get either
        // the old or the new payload, never a truncated one).
        let tmp = state_path.with_extension("json.tmp");
        let _ = fs::remove_file(&tmp);
        fs::write(&tmp, content.as_bytes())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
        }
        fs::rename(&tmp, &state_path)?;
        Ok(())
    }

    /// Add or update a tool in the state. `version` here is the
    /// **config constraint** (`"latest"`, `"^20"`, `"20.10.0"`).
    /// Drift baseline capture should prefer `set_tool_with_installed`
    /// so the concrete probed version is recorded and check-time
    /// comparisons don't false-positive.
    pub fn set_tool(&mut self, name: &str, version: &str, path: &Path, install_method: &str) {
        self.set_tool_with_installed(name, version, None, path, install_method);
    }

    /// Add or update a tool with an explicit installed version. When
    /// `installed_version` is `Some(v)`, the drift detector compares
    /// against `v` instead of the config constraint — this is the
    /// setup-time write path that avoids the "config says 'latest',
    /// probe returns '20.10.0'" false-positive loop.
    pub fn set_tool_with_installed(
        &mut self,
        name: &str,
        version: &str,
        installed_version: Option<&str>,
        path: &Path,
        install_method: &str,
    ) {
        self.tools.insert(
            name.to_string(),
            ToolState {
                version: version.to_string(),
                installed_version: installed_version.map(str::to_string),
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
        assert_eq!(tool.installed_version, None);
        assert_eq!(tool.install_method, "brew");
    }

    #[test]
    fn set_tool_with_installed_records_concrete_version() {
        let mut state = EnvironmentState::new();
        state.set_tool_with_installed(
            "node",
            "^20",
            Some("20.10.0"),
            Path::new("/usr/bin/node"),
            "brew",
        );

        let tool = state.tools.get("node").unwrap();
        // Config constraint preserved for parity with jarvy.toml,
        // concrete probed version recorded separately (Codex P1 fix).
        assert_eq!(tool.version, "^20");
        assert_eq!(tool.installed_version.as_deref(), Some("20.10.0"));
    }

    /// Legacy `state.json` files (pre P1 fix) omit `installed_version`.
    /// Confirm they still deserialize via `#[serde(default)]` so the
    /// migration is transparent — users on an old baseline get
    /// bounded false-positives (the pre-fix behavior) until they
    /// re-run `jarvy setup`, not a hard parse error.
    #[test]
    fn legacy_state_without_installed_version_field_still_loads() {
        let json = r#"{
            "version": "1",
            "created_at": "0Z",
            "updated_at": "0Z",
            "config_hash": "",
            "tools": {
                "node": {
                    "version": "20.10.0",
                    "path": "/usr/bin/node",
                    "install_method": "brew"
                }
            },
            "files": {}
        }"#;
        let state: EnvironmentState = serde_json::from_str(json).expect("legacy shape must load");
        let node = state.tools.get("node").unwrap();
        assert_eq!(node.version, "20.10.0");
        assert_eq!(node.installed_version, None);
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

        // Verify file exists (using the canonical resolver path).
        let state_path = crate::paths::state_json(project_dir);
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
