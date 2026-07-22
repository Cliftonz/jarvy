//! Lock file generation
//!
//! Generates jarvy.lock from the current environment.

use std::collections::HashMap;
use std::process::Command;

use super::{InstallSource, LockError, LockFile, LockMeta, LockedTool};
use crate::config::Tool;
use crate::tools::common::{Os, has};
use crate::tools::spec::get_tool_spec;

/// Generate a lock file from configured tools
pub fn generate_lock(tools: &HashMap<String, Tool>) -> Result<LockFile, LockError> {
    let mut lock = LockFile {
        version: super::LOCK_VERSION.to_string(),
        meta: LockMeta::default(),
        tools: HashMap::new(),
        platforms: HashMap::new(),
    };

    for name in tools.keys() {
        if let Some(locked) = lock_tool(name) {
            lock.tools.insert(name.clone(), locked);
        }
    }

    Ok(lock)
}

/// Lock a single tool by detecting its installed version and source
fn lock_tool(name: &str) -> Option<LockedTool> {
    // Check if tool is installed
    if !has(name) {
        return None;
    }

    // Get installed version
    let version = get_installed_version(name)?;

    // Determine install source
    let source = detect_install_source(name);

    // Get binary path
    let binary_path = get_binary_path(name);

    // Compute checksum (optional, expensive)
    let checksum = binary_path.as_ref().and_then(|p| compute_checksum(p).ok());

    Some(LockedTool {
        version,
        source,
        checksum,
        binary_path,
    })
}

/// Get installed version of a tool
pub fn get_installed_version(name: &str) -> Option<String> {
    // Try common version flags
    let version_args = ["--version", "-v", "version", "-V"];

    for arg in version_args {
        if let Some(version) = try_get_version(name, arg) {
            return Some(version);
        }
    }

    None
}

/// Try to get version with a specific argument
fn try_get_version(cmd: &str, arg: &str) -> Option<String> {
    let output = Command::new(cmd).arg(arg).output().ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    extract_version(&stdout)
}

/// Extract version string from command output. Delegates to the
/// canonical extractor in `tools::version` so lock-file output and
/// drift reports normalize identical `--version` output to identical
/// strings (round-2 maint F14).
fn extract_version(output: &str) -> Option<String> {
    crate::tools::version::extract_version(output).map(|v| v.to_string())
}

/// Detect how a tool was installed
fn detect_install_source(name: &str) -> InstallSource {
    let os = crate::tools::current_os();

    match os {
        Os::Macos => detect_macos_source(name),
        Os::Linux => detect_linux_source(name),
        Os::Windows => detect_windows_source(name),
        Os::Bsd => detect_bsd_source(name),
    }
}

/// Detect install source on macOS
fn detect_macos_source(name: &str) -> InstallSource {
    // Check if installed via Homebrew
    if let Some(path) = get_binary_path(name)
        && (path.contains("/opt/homebrew/") || path.contains("/usr/local/Cellar/"))
    {
        // Check if it's a cask
        if is_brew_cask(name) {
            return InstallSource::BrewCask;
        }
        return InstallSource::Brew;
    }

    // Check for custom installers
    if let Some(spec) = get_tool_spec(name)
        && spec.custom_install.is_some()
    {
        return InstallSource::Custom(name.to_string());
    }

    InstallSource::Unknown
}

/// Check if a tool is a Homebrew cask
fn is_brew_cask(name: &str) -> bool {
    if let Some(spec) = get_tool_spec(name)
        && let Some(macos) = &spec.macos
    {
        return macos.cask.is_some();
    }
    false
}

/// Detect install source on Linux
fn detect_linux_source(name: &str) -> InstallSource {
    // Check dpkg (Debian/Ubuntu)
    if command_succeeds("dpkg", &["-s", name]) {
        return InstallSource::Apt;
    }

    // Check rpm (Fedora/RHEL)
    if command_succeeds("rpm", &["-q", name]) {
        return InstallSource::Dnf;
    }

    // Check pacman (Arch)
    if command_succeeds("pacman", &["-Q", name]) {
        return InstallSource::Pacman;
    }

    // Check apk (Alpine)
    if command_succeeds("apk", &["info", "-e", name]) {
        return InstallSource::Apk;
    }

    // Check for custom installers
    if let Some(spec) = get_tool_spec(name)
        && spec.custom_install.is_some()
    {
        return InstallSource::Custom(name.to_string());
    }

    InstallSource::Unknown
}

/// Detect install source on Windows
fn detect_windows_source(name: &str) -> InstallSource {
    // Check winget
    if command_succeeds("winget", &["list", "--id", name]) {
        return InstallSource::Winget;
    }

    // Check chocolatey
    if command_succeeds("choco", &["list", "--local-only", name]) {
        return InstallSource::Choco;
    }

    // Check for custom installers
    if let Some(spec) = get_tool_spec(name)
        && spec.custom_install.is_some()
    {
        return InstallSource::Custom(name.to_string());
    }

    InstallSource::Unknown
}

/// Detect install source on BSD (FreeBSD)
fn detect_bsd_source(name: &str) -> InstallSource {
    // Check pkg (FreeBSD package manager)
    if command_succeeds("pkg", &["info", name]) {
        return InstallSource::Pkg;
    }

    // Check for custom installers
    if let Some(spec) = get_tool_spec(name)
        && spec.custom_install.is_some()
    {
        return InstallSource::Custom(name.to_string());
    }

    InstallSource::Unknown
}

/// Check if a command succeeds
fn command_succeeds(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get the binary path for a command
fn get_binary_path(name: &str) -> Option<String> {
    #[cfg(unix)]
    {
        let output = Command::new("which").arg(name).output().ok()?;

        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }

    #[cfg(windows)]
    {
        let output = Command::new("where").arg(name).output().ok()?;

        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .map(|s| s.trim().to_string());
            return path;
        }
    }

    None
}

/// Returns true when the given checksum string is in the legacy short-hash
/// format that earlier jarvy versions wrote (FNV-style 16-char hex). Lock
/// files containing such hashes are still readable but cannot be re-verified
/// against the new SHA-256 output.
pub(crate) fn is_legacy_checksum(checksum: &str) -> bool {
    checksum.len() == 16 && checksum.chars().all(|c| c.is_ascii_hexdigit())
}

/// Returns true for a well-formed SHA-256 hex digest (lowercase or upper).
#[allow(dead_code)] // Public API — exposed for migration callers and tests
pub(crate) fn is_sha256_checksum(checksum: &str) -> bool {
    checksum.len() == 64 && checksum.chars().all(|c| c.is_ascii_hexdigit())
}

/// Compute SHA-256 checksum of a file
fn compute_checksum(path: &str) -> Result<String, LockError> {
    use sha2::{Digest, Sha256};
    use std::fs::File;
    use std::io::Read;

    let mut file = File::open(path).map_err(|e| LockError::IoError {
        path: path.to_string(),
        error: e.to_string(),
    })?;

    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer).map_err(|e| LockError::IoError {
            path: path.to_string(),
            error: e.to_string(),
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_version_semver() {
        assert_eq!(
            extract_version("git version 2.45.0"),
            Some("2.45.0".to_string())
        );
    }

    #[test]
    fn test_extract_version_with_v_prefix() {
        assert_eq!(
            extract_version("node v20.10.0"),
            Some("20.10.0".to_string())
        );
    }

    #[test]
    fn test_extract_version_two_parts() {
        // Canonical normalizer (tools::version) zero-fills the patch
        // component so "3.12" → "3.12.0". Wire format is now consistent
        // across drift / lock / doctor (round-2 maint F14).
        assert_eq!(extract_version("python 3.12"), Some("3.12.0".to_string()));
    }

    #[test]
    fn test_extract_version_with_suffix() {
        assert_eq!(
            extract_version("rustc 1.75.0-beta.1"),
            Some("1.75.0-beta.1".to_string())
        );
    }

    #[test]
    fn test_extract_version_no_match() {
        assert_eq!(extract_version("no version here"), None);
    }

    #[test]
    fn compute_checksum_known_vector_empty_file() {
        let tmp = tempfile::NamedTempFile::new().expect("create tempfile");
        // Empty file: well-known SHA-256.
        let path = tmp.path().to_str().unwrap();
        let hash = compute_checksum(path).expect("hash empty file");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert!(is_sha256_checksum(&hash));
    }

    #[test]
    fn compute_checksum_known_vector_hello_world() {
        let mut tmp = tempfile::NamedTempFile::new().expect("create tempfile");
        std::io::Write::write_all(&mut tmp, b"hello world").expect("write");
        let path = tmp.path().to_str().unwrap();
        let hash = compute_checksum(path).expect("hash hello world");
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn compute_checksum_streams_large_file_consistently() {
        // Larger than the 8KiB read buffer to exercise the streaming loop.
        let mut tmp = tempfile::NamedTempFile::new().expect("create tempfile");
        let chunk = vec![0xABu8; 8192];
        for _ in 0..3 {
            std::io::Write::write_all(&mut tmp, &chunk).expect("write");
        }
        std::io::Write::write_all(&mut tmp, b"trailing").expect("write trailing");
        let path = tmp.path().to_str().unwrap();
        let hash_a = compute_checksum(path).unwrap();
        let hash_b = compute_checksum(path).unwrap();
        assert_eq!(hash_a, hash_b, "checksum must be deterministic");
        assert_eq!(hash_a.len(), 64);
    }

    #[test]
    fn legacy_checksum_detection() {
        // Old FNV-style 16-char hex.
        assert!(is_legacy_checksum("0123456789abcdef"));
        assert!(!is_legacy_checksum(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        ));
        assert!(!is_legacy_checksum(""));
        assert!(!is_legacy_checksum("nothex0123456789"));
    }

    #[test]
    fn sha256_checksum_detection() {
        assert!(is_sha256_checksum(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        ));
        // Wrong length.
        assert!(!is_sha256_checksum("0123456789abcdef"));
        // Correct length but non-hex content.
        assert!(!is_sha256_checksum(&"x".repeat(64)));
    }
}
