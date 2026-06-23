//! Installation method detection and update execution
//!
//! Detects how Jarvy was installed (Homebrew, Cargo, apt, etc.)
//! and executes updates via the appropriate package manager.

#![allow(dead_code)] // Public API for installation method detection

use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};
#[allow(unused_imports)] // Stdio used only on Linux/Windows for package manager checks
use std::process::{Command, Stdio};

/// Installation method for Jarvy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallMethod {
    /// Homebrew on macOS
    Homebrew,
    /// Cargo install (Rust)
    Cargo,
    /// apt package manager (Debian/Ubuntu)
    Apt,
    /// dnf package manager (Fedora/RHEL)
    Dnf,
    /// pacman package manager (Arch)
    Pacman,
    /// winget package manager (Windows)
    Winget,
    /// Chocolatey package manager (Windows)
    Chocolatey,
    /// Scoop package manager (Windows)
    Scoop,
    /// Direct binary installation
    Binary,
    /// Unknown installation method
    Unknown,
}

impl InstallMethod {
    /// Detect installation method from binary path and system queries
    pub fn detect() -> Self {
        // First check cached method
        if let Some(cached) = Self::load_cached() {
            // Verify the cache is still valid (binary hasn't moved)
            if Self::verify_cache_valid() {
                return cached;
            }
        }

        let method = Self::detect_fresh();

        // Cache the detected method
        if method != InstallMethod::Unknown {
            let _ = method.cache();
        }

        method
    }

    /// Detect installation method without cache
    fn detect_fresh() -> Self {
        // Get current binary path
        let binary_path = match env::current_exe() {
            Ok(path) => path,
            Err(_) => return InstallMethod::Unknown,
        };

        // Path-based detection
        if let Some(method) = Self::detect_from_path(&binary_path) {
            return method;
        }

        // Package manager query detection
        if let Some(method) = Self::detect_from_package_managers(&binary_path) {
            return method;
        }

        // Default to binary if we can't determine
        InstallMethod::Binary
    }

    /// Detect from binary path patterns
    fn detect_from_path(path: &Path) -> Option<Self> {
        let path_str = path.to_string_lossy();

        // macOS Homebrew paths
        if path_str.contains("/opt/homebrew/") || path_str.contains("/usr/local/Cellar/") {
            return Some(InstallMethod::Homebrew);
        }

        // Cargo install path
        if path_str.contains(".cargo/bin") {
            return Some(InstallMethod::Cargo);
        }

        // Windows package managers
        #[cfg(windows)]
        {
            if path_str.contains("\\scoop\\") || path_str.contains("/scoop/") {
                return Some(InstallMethod::Scoop);
            }
            if path_str.contains("\\chocolatey\\") || path_str.contains("/chocolatey/") {
                return Some(InstallMethod::Chocolatey);
            }
            // winget typically installs to Program Files
            if path_str.contains("\\WindowsApps\\") || path_str.contains("\\Program Files\\") {
                return Some(InstallMethod::Winget);
            }
        }

        // Linux system paths (likely from package manager)
        #[cfg(target_os = "linux")]
        {
            if path_str.starts_with("/usr/bin/") || path_str.starts_with("/usr/local/bin/") {
                // Could be apt, dnf, or pacman - need to query
                return None;
            }
        }

        None
    }

    /// Detect from package manager queries
    #[allow(unused_variables)] // binary_path only used on Linux/Windows
    fn detect_from_package_managers(binary_path: &Path) -> Option<Self> {
        // Linux: Check dpkg (Debian/Ubuntu)
        #[cfg(target_os = "linux")]
        {
            let path_str = binary_path.to_string_lossy();
            if Self::check_dpkg(&path_str) {
                return Some(InstallMethod::Apt);
            }

            if Self::check_rpm(&path_str) {
                return Some(InstallMethod::Dnf);
            }

            if Self::check_pacman() {
                return Some(InstallMethod::Pacman);
            }
        }

        // Windows: Check winget
        #[cfg(windows)]
        {
            if Self::check_winget() {
                return Some(InstallMethod::Winget);
            }
        }

        None
    }

    #[cfg(target_os = "linux")]
    fn check_dpkg(path: &str) -> bool {
        Command::new("dpkg")
            .args(["-S", path])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[cfg(target_os = "linux")]
    fn check_rpm(path: &str) -> bool {
        Command::new("rpm")
            .args(["-qf", path])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[cfg(target_os = "linux")]
    fn check_pacman() -> bool {
        Command::new("pacman")
            .args(["-Qs", "jarvy"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[cfg(windows)]
    fn check_winget() -> bool {
        let output = Command::new("winget")
            .args(["list", "--name", "jarvy"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                stdout.to_lowercase().contains("jarvy")
            }
            Err(_) => false,
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn check_dpkg(_path: &str) -> bool {
        false
    }

    #[cfg(not(target_os = "linux"))]
    fn check_rpm(_path: &str) -> bool {
        false
    }

    #[cfg(not(target_os = "linux"))]
    fn check_pacman() -> bool {
        false
    }

    #[cfg(not(windows))]
    fn check_winget() -> bool {
        false
    }

    /// Load cached installation method
    fn load_cached() -> Option<Self> {
        let cache_path = Self::cache_path()?;
        let content = std::fs::read_to_string(cache_path).ok()?;

        #[derive(Deserialize)]
        struct CacheEntry {
            method: InstallMethod,
            binary_path: String,
        }

        let entry: CacheEntry = serde_json::from_str(&content).ok()?;

        // Verify binary path matches
        let current_path = env::current_exe().ok()?;
        if current_path.to_string_lossy() == entry.binary_path {
            Some(entry.method)
        } else {
            None
        }
    }

    /// Verify the cached method is still valid
    fn verify_cache_valid() -> bool {
        // Already checked in load_cached via binary_path comparison
        true
    }

    /// Cache the installation method
    fn cache(&self) -> std::io::Result<()> {
        let cache_path = Self::cache_path()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No cache path"))?;

        let binary_path = env::current_exe()?;

        #[derive(Serialize)]
        struct CacheEntry {
            method: InstallMethod,
            binary_path: String,
        }

        let entry = CacheEntry {
            method: *self,
            binary_path: binary_path.to_string_lossy().to_string(),
        };

        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(&entry)?;
        std::fs::write(cache_path, content)
    }

    /// Get cache file path (canonical resolver in `crate::paths`).
    fn cache_path() -> Option<PathBuf> {
        crate::paths::install_method_json().ok()
    }

    /// Parse installation method from string. Returns `None` for unknown
    /// methods rather than `Err`, so this is intentionally not a
    /// `std::str::FromStr` impl (which would force `Err` on every lookup
    /// where the caller just wants `Option`).
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "homebrew" | "brew" => Some(InstallMethod::Homebrew),
            "cargo" => Some(InstallMethod::Cargo),
            "apt" | "apt-get" => Some(InstallMethod::Apt),
            "dnf" | "yum" => Some(InstallMethod::Dnf),
            "pacman" => Some(InstallMethod::Pacman),
            "winget" => Some(InstallMethod::Winget),
            "chocolatey" | "choco" => Some(InstallMethod::Chocolatey),
            "scoop" => Some(InstallMethod::Scoop),
            "binary" => Some(InstallMethod::Binary),
            _ => None,
        }
    }

    /// Get method name as string
    pub fn as_str(&self) -> &'static str {
        match self {
            InstallMethod::Homebrew => "homebrew",
            InstallMethod::Cargo => "cargo",
            InstallMethod::Apt => "apt",
            InstallMethod::Dnf => "dnf",
            InstallMethod::Pacman => "pacman",
            InstallMethod::Winget => "winget",
            InstallMethod::Chocolatey => "chocolatey",
            InstallMethod::Scoop => "scoop",
            InstallMethod::Binary => "binary",
            InstallMethod::Unknown => "unknown",
        }
    }

    /// Execute update via this installation method
    pub fn execute_update(&self, version: Option<&str>) -> Result<(), UpdateError> {
        match self {
            InstallMethod::Homebrew => self.update_homebrew(),
            InstallMethod::Cargo => self.update_cargo(version),
            InstallMethod::Apt => self.update_apt(),
            InstallMethod::Dnf => self.update_dnf(),
            InstallMethod::Pacman => self.update_pacman(),
            InstallMethod::Winget => self.update_winget(),
            InstallMethod::Chocolatey => self.update_chocolatey(),
            InstallMethod::Scoop => self.update_scoop(),
            InstallMethod::Binary | InstallMethod::Unknown => {
                Err(UpdateError::MethodUnsupported(*self))
            }
        }
    }

    fn update_homebrew(&self) -> Result<(), UpdateError> {
        let status = Command::new("brew")
            .args(["upgrade", "jarvy"])
            .status()
            .map_err(|e| UpdateError::ExecutionFailed(format!("brew upgrade: {}", e)))?;

        if status.success() {
            Ok(())
        } else {
            Err(UpdateError::ExecutionFailed(
                "brew upgrade jarvy failed".to_string(),
            ))
        }
    }

    fn update_cargo(&self, version: Option<&str>) -> Result<(), UpdateError> {
        let mut cmd = Command::new("cargo");
        cmd.args(["install", "jarvy"]);

        if let Some(v) = version {
            cmd.args(["--version", v]);
        }

        let status = cmd
            .status()
            .map_err(|e| UpdateError::ExecutionFailed(format!("cargo install: {}", e)))?;

        if status.success() {
            Ok(())
        } else {
            Err(UpdateError::ExecutionFailed(
                "cargo install jarvy failed".to_string(),
            ))
        }
    }

    fn update_apt(&self) -> Result<(), UpdateError> {
        // First update package lists
        let update_status = Command::new("sudo")
            .args(["apt-get", "update"])
            .status()
            .map_err(|e| UpdateError::ExecutionFailed(format!("apt update: {}", e)))?;

        if !update_status.success() {
            return Err(UpdateError::ExecutionFailed(
                "apt-get update failed".to_string(),
            ));
        }

        // Then upgrade jarvy
        let status = Command::new("sudo")
            .args(["apt-get", "install", "--only-upgrade", "-y", "jarvy"])
            .status()
            .map_err(|e| UpdateError::ExecutionFailed(format!("apt upgrade: {}", e)))?;

        if status.success() {
            Ok(())
        } else {
            Err(UpdateError::ExecutionFailed(
                "apt-get upgrade jarvy failed".to_string(),
            ))
        }
    }

    fn update_dnf(&self) -> Result<(), UpdateError> {
        let status = Command::new("sudo")
            .args(["dnf", "upgrade", "-y", "jarvy"])
            .status()
            .map_err(|e| UpdateError::ExecutionFailed(format!("dnf upgrade: {}", e)))?;

        if status.success() {
            Ok(())
        } else {
            Err(UpdateError::ExecutionFailed(
                "dnf upgrade jarvy failed".to_string(),
            ))
        }
    }

    fn update_pacman(&self) -> Result<(), UpdateError> {
        let status = Command::new("sudo")
            .args(["pacman", "-Syu", "--noconfirm", "jarvy"])
            .status()
            .map_err(|e| UpdateError::ExecutionFailed(format!("pacman upgrade: {}", e)))?;

        if status.success() {
            Ok(())
        } else {
            Err(UpdateError::ExecutionFailed(
                "pacman upgrade jarvy failed".to_string(),
            ))
        }
    }

    fn update_winget(&self) -> Result<(), UpdateError> {
        let status = Command::new("winget")
            .args(["upgrade", "jarvy"])
            .status()
            .map_err(|e| UpdateError::ExecutionFailed(format!("winget upgrade: {}", e)))?;

        if status.success() {
            Ok(())
        } else {
            Err(UpdateError::ExecutionFailed(
                "winget upgrade jarvy failed".to_string(),
            ))
        }
    }

    fn update_chocolatey(&self) -> Result<(), UpdateError> {
        let status = Command::new("choco")
            .args(["upgrade", "jarvy", "-y"])
            .status()
            .map_err(|e| UpdateError::ExecutionFailed(format!("choco upgrade: {}", e)))?;

        if status.success() {
            Ok(())
        } else {
            Err(UpdateError::ExecutionFailed(
                "choco upgrade jarvy failed".to_string(),
            ))
        }
    }

    fn update_scoop(&self) -> Result<(), UpdateError> {
        let status = Command::new("scoop")
            .args(["update", "jarvy"])
            .status()
            .map_err(|e| UpdateError::ExecutionFailed(format!("scoop update: {}", e)))?;

        if status.success() {
            Ok(())
        } else {
            Err(UpdateError::ExecutionFailed(
                "scoop update jarvy failed".to_string(),
            ))
        }
    }

    /// Check if this method supports direct updates
    pub fn supports_direct_update(&self) -> bool {
        !matches!(self, InstallMethod::Binary | InstallMethod::Unknown)
    }
}

impl std::fmt::Display for InstallMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Errors during update execution
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("Unsupported installation method: {0}")]
    MethodUnsupported(InstallMethod),

    #[error("Update execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Binary download failed: {0}")]
    DownloadFailed(String),

    #[error("Checksum verification failed")]
    ChecksumMismatch,

    #[error("Binary installation failed: {0}")]
    InstallationFailed(String),

    #[error("Rollback failed: {0}")]
    RollbackFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_method_from_str() {
        assert_eq!(
            InstallMethod::from_str("homebrew"),
            Some(InstallMethod::Homebrew)
        );
        assert_eq!(
            InstallMethod::from_str("brew"),
            Some(InstallMethod::Homebrew)
        );
        assert_eq!(InstallMethod::from_str("CARGO"), Some(InstallMethod::Cargo));
        assert_eq!(InstallMethod::from_str("apt"), Some(InstallMethod::Apt));
        assert_eq!(InstallMethod::from_str("invalid"), None);
    }

    #[test]
    fn test_install_method_as_str() {
        assert_eq!(InstallMethod::Homebrew.as_str(), "homebrew");
        assert_eq!(InstallMethod::Cargo.as_str(), "cargo");
        assert_eq!(InstallMethod::Binary.as_str(), "binary");
    }

    #[test]
    fn test_supports_direct_update() {
        assert!(InstallMethod::Homebrew.supports_direct_update());
        assert!(InstallMethod::Cargo.supports_direct_update());
        assert!(!InstallMethod::Binary.supports_direct_update());
        assert!(!InstallMethod::Unknown.supports_direct_update());
    }
}
