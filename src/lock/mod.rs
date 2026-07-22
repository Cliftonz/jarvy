//! Version Lock File Module (PRD-024)
//!
//! Provides version locking for reproducible environments:
//! - Lock file generation from current environment
//! - Lock file verification
//! - Platform-specific lock sections

pub mod generate;
pub mod verify;

pub use generate::generate_lock;
#[allow(unused_imports)]
pub use verify::{VerificationResult, VerificationStatus, verify_lock};

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Lock file version for format compatibility
pub const LOCK_VERSION: &str = "1";

/// Lock file name
#[allow(dead_code)] // Public constant for lock file operations
pub const LOCK_FILE_NAME: &str = "jarvy.lock";

/// Lock file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockFile {
    /// Lock file format version
    pub version: String,
    /// Metadata about lock generation
    pub meta: LockMeta,
    /// Locked tools with versions
    pub tools: HashMap<String, LockedTool>,
    /// Platform-specific tool overrides
    #[serde(default)]
    pub platforms: HashMap<String, HashMap<String, LockedTool>>,
}

impl LockFile {
    /// Create a new lock file
    pub fn new() -> Self {
        Self {
            version: LOCK_VERSION.to_string(),
            meta: LockMeta::default(),
            tools: HashMap::new(),
            platforms: HashMap::new(),
        }
    }

    /// Load lock file from path
    pub fn load(path: &Path) -> Result<Self, LockError> {
        let content = fs::read_to_string(path).map_err(|e| LockError::IoError {
            path: path.display().to_string(),
            error: e.to_string(),
        })?;

        toml::from_str(&content).map_err(|e| LockError::ParseError {
            path: path.display().to_string(),
            error: e.to_string(),
        })
    }

    /// Save lock file to path
    pub fn save(&self, path: &Path) -> Result<(), LockError> {
        let content = toml::to_string_pretty(self).map_err(|e| LockError::SerializeError {
            error: e.to_string(),
        })?;

        fs::write(path, content).map_err(|e| LockError::IoError {
            path: path.display().to_string(),
            error: e.to_string(),
        })
    }

    /// Get locked version for a tool, considering platform overrides
    pub fn get_tool(&self, name: &str, platform: &str) -> Option<&LockedTool> {
        // Check platform-specific first
        if let Some(platform_tools) = self.platforms.get(platform)
            && let Some(tool) = platform_tools.get(name)
        {
            return Some(tool);
        }
        // Fall back to common
        self.tools.get(name)
    }

    /// Add or update a tool
    #[allow(dead_code)] // Public API for lock file manipulation
    pub fn set_tool(&mut self, name: &str, tool: LockedTool) {
        self.tools.insert(name.to_string(), tool);
    }

    /// Add or update a platform-specific tool
    #[allow(dead_code)] // Public API for lock file manipulation
    pub fn set_platform_tool(&mut self, platform: &str, name: &str, tool: LockedTool) {
        self.platforms
            .entry(platform.to_string())
            .or_default()
            .insert(name.to_string(), tool);
    }
}

impl Default for LockFile {
    fn default() -> Self {
        Self::new()
    }
}

/// Metadata about lock file generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockMeta {
    /// Timestamp when lock was generated (Unix)
    pub generated: u64,
    /// Jarvy version used to generate
    pub jarvy_version: String,
    /// Platform this was generated on
    pub platform: String,
    /// Architecture
    pub arch: String,
}

impl Default for LockMeta {
    fn default() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};

        Self {
            generated: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            jarvy_version: env!("CARGO_PKG_VERSION").to_string(),
            platform: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
        }
    }
}

/// A locked tool entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedTool {
    /// Exact version string
    pub version: String,
    /// How this tool was installed
    pub source: InstallSource,
    /// SHA256 checksum of the binary (optional)
    #[serde(default)]
    pub checksum: Option<String>,
    /// Binary path at time of lock (informational)
    #[serde(default)]
    pub binary_path: Option<String>,
}

/// How a tool was installed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum InstallSource {
    /// Homebrew
    Brew,
    /// Homebrew Cask
    BrewCask,
    /// APT
    Apt,
    /// DNF/YUM
    Dnf,
    /// Pacman
    Pacman,
    /// APK (Alpine)
    Apk,
    /// Pkg (FreeBSD)
    Pkg,
    /// Winget
    Winget,
    /// Chocolatey
    Choco,
    /// Custom installer (nvm, rustup, etc.)
    Custom(String),
    /// Unknown/manual installation
    Unknown,
}

impl std::fmt::Display for InstallSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallSource::Brew => write!(f, "brew"),
            InstallSource::BrewCask => write!(f, "brew-cask"),
            InstallSource::Apt => write!(f, "apt"),
            InstallSource::Dnf => write!(f, "dnf"),
            InstallSource::Pacman => write!(f, "pacman"),
            InstallSource::Apk => write!(f, "apk"),
            InstallSource::Pkg => write!(f, "pkg"),
            InstallSource::Winget => write!(f, "winget"),
            InstallSource::Choco => write!(f, "choco"),
            InstallSource::Custom(name) => write!(f, "custom:{}", name),
            InstallSource::Unknown => write!(f, "unknown"),
        }
    }
}

/// Lock file errors
#[derive(Debug)]
pub enum LockError {
    /// I/O error
    IoError { path: String, error: String },
    /// Parse error
    ParseError { path: String, error: String },
    /// Serialization error
    SerializeError { error: String },
    /// Tool not found
    #[allow(dead_code)] // Reserved for lock verification
    ToolNotFound { name: String },
    /// Version mismatch
    #[allow(dead_code)] // Reserved for lock verification
    VersionMismatch {
        tool: String,
        locked: String,
        installed: String,
    },
    /// Checksum mismatch
    #[allow(dead_code)] // Reserved for lock verification
    ChecksumMismatch {
        tool: String,
        locked: String,
        computed: String,
    },
}

impl std::fmt::Display for LockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LockError::IoError { path, error } => {
                write!(f, "I/O error at '{}': {}", path, error)
            }
            LockError::ParseError { path, error } => {
                write!(f, "Failed to parse lock file '{}': {}", path, error)
            }
            LockError::SerializeError { error } => {
                write!(f, "Failed to serialize lock file: {}", error)
            }
            LockError::ToolNotFound { name } => {
                write!(f, "Tool '{}' not found in lock file", name)
            }
            LockError::VersionMismatch {
                tool,
                locked,
                installed,
            } => {
                write!(
                    f,
                    "Version mismatch for '{}': locked={}, installed={}",
                    tool, locked, installed
                )
            }
            LockError::ChecksumMismatch {
                tool,
                locked,
                computed,
            } => {
                write!(
                    f,
                    "Checksum mismatch for '{}': locked={}, computed={}",
                    tool, locked, computed
                )
            }
        }
    }
}

impl std::error::Error for LockError {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_lock_file_new() {
        let lock = LockFile::new();
        assert_eq!(lock.version, LOCK_VERSION);
        assert!(lock.tools.is_empty());
    }

    #[test]
    fn test_lock_file_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("jarvy.lock");

        let mut lock = LockFile::new();
        lock.set_tool(
            "git",
            LockedTool {
                version: "2.45.0".to_string(),
                source: InstallSource::Brew,
                checksum: None,
                binary_path: Some("/opt/homebrew/bin/git".to_string()),
            },
        );

        lock.save(&path).unwrap();
        assert!(path.exists());

        let loaded = LockFile::load(&path).unwrap();
        assert_eq!(loaded.tools.len(), 1);
        let git = loaded.tools.get("git").unwrap();
        assert_eq!(git.version, "2.45.0");
        assert_eq!(git.source, InstallSource::Brew);
    }

    #[test]
    fn test_platform_specific_tools() {
        let mut lock = LockFile::new();

        // Common tool
        lock.set_tool(
            "git",
            LockedTool {
                version: "2.45.0".to_string(),
                source: InstallSource::Brew,
                checksum: None,
                binary_path: None,
            },
        );

        // Platform-specific override
        lock.set_platform_tool(
            "linux",
            "git",
            LockedTool {
                version: "2.40.0".to_string(),
                source: InstallSource::Apt,
                checksum: None,
                binary_path: None,
            },
        );

        // macos should get common
        let macos_git = lock.get_tool("git", "macos").unwrap();
        assert_eq!(macos_git.version, "2.45.0");

        // linux should get override
        let linux_git = lock.get_tool("git", "linux").unwrap();
        assert_eq!(linux_git.version, "2.40.0");
    }

    #[test]
    fn test_install_source_display() {
        assert_eq!(InstallSource::Brew.to_string(), "brew");
        assert_eq!(
            InstallSource::Custom("nvm".to_string()).to_string(),
            "custom:nvm"
        );
    }

    #[test]
    fn test_lock_meta_default() {
        let meta = LockMeta::default();
        assert!(!meta.jarvy_version.is_empty());
        assert!(!meta.platform.is_empty());
        assert!(!meta.arch.is_empty());
    }
}
