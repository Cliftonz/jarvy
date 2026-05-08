//! Binary installer for direct download updates
//!
//! Downloads and installs Jarvy binaries directly from GitHub releases
//! when package manager updates are not available.

#![allow(dead_code)] // Public API for binary installation

use crate::update::method::UpdateError;
use crate::update::release::{GitHubRelease, ReleaseAsset};
use crate::update::rollback::RollbackManager;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// Binary installer for direct updates
pub struct BinaryInstaller {
    /// Backup directory for rollback support
    backup_dir: PathBuf,
    /// Staging directory for downloads
    staging_dir: PathBuf,
}

impl BinaryInstaller {
    /// Create a new binary installer
    pub fn new() -> io::Result<Self> {
        let jarvy_dir = dirs::home_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No home directory"))?
            .join(".jarvy");

        let backup_dir = jarvy_dir.join("backup");
        let staging_dir = jarvy_dir.join("staging");

        fs::create_dir_all(&backup_dir)?;
        fs::create_dir_all(&staging_dir)?;

        Ok(Self {
            backup_dir,
            staging_dir,
        })
    }

    /// Install a release via direct binary download.
    ///
    /// Defaults to fail-closed Sigstore signature verification. Pass
    /// `allow_unsigned = true` only when the operator has explicitly opted
    /// into unsigned updates via the `--allow-unsigned` CLI flag or
    /// `JARVY_ALLOW_UNSIGNED_UPDATE=1` env var.
    pub fn install(&self, release: &GitHubRelease) -> Result<InstallResult, UpdateError> {
        self.install_with_options(release, false)
    }

    /// Install a release with explicit signature-policy control.
    pub fn install_with_options(
        &self,
        release: &GitHubRelease,
        allow_unsigned: bool,
    ) -> Result<InstallResult, UpdateError> {
        // Get current binary path
        let current_exe = std::env::current_exe().map_err(|e| {
            UpdateError::InstallationFailed(format!("Cannot find current exe: {}", e))
        })?;

        // Find the appropriate asset for this platform
        let asset = release.asset_for_platform().ok_or_else(|| {
            UpdateError::DownloadFailed("No binary for this platform".to_string())
        })?;

        println!(
            "Downloading jarvy v{} for {}...",
            release.version(),
            crate::update::release::get_current_target()
        );

        // Download the binary archive
        let archive_path = self.download_asset(asset)?;

        // Verify checksum if available
        if let Some(checksum_asset) = release.checksum_asset() {
            println!("Verifying checksum...");
            let checksums = self.download_checksums(checksum_asset)?;
            self.verify_archive_checksum(&archive_path, &asset.name, &checksums)?;
        }

        // Verify Sigstore signature — fail closed unless the operator has
        // explicitly opted into unsigned updates.
        let allow_unsigned = allow_unsigned || super::signature::unsigned_override_from_env();
        let outcome = super::signature::verify_sigstore_signature(&archive_path)
            .map_err(|e| UpdateError::InstallationFailed(format!("signature error: {e}")))?;
        if let Err(reason) =
            super::signature::signature_outcome_is_acceptable(&outcome, allow_unsigned)
        {
            eprintln!("\x1b[31m[SECURITY]\x1b[0m {reason}");
            return Err(UpdateError::InstallationFailed(reason));
        }
        if matches!(outcome, super::signature::SignatureOutcome::Verified) {
            println!(
                "\x1b[32m[VERIFIED]\x1b[0m Sigstore signature verified for {}",
                archive_path.display()
            );
        }

        // Extract the binary
        println!("Extracting binary...");
        let binary_path = self.extract_binary(&archive_path)?;

        // Backup current binary
        println!("Backing up current version...");
        let backup_path = self.backup_current(&current_exe)?;

        // Replace with new binary
        println!("Installing update...");
        self.replace_binary(&binary_path, &current_exe)?;

        // Verify new binary works
        if !self.verify_installation(&current_exe)? {
            // Rollback on failure
            eprintln!("Installation verification failed, rolling back...");
            self.restore_backup(&backup_path, &current_exe)?;
            return Err(UpdateError::InstallationFailed(
                "New binary verification failed".to_string(),
            ));
        }

        // Record rollback info
        RollbackManager::record_update(env!("CARGO_PKG_VERSION"), release.version(), &backup_path)?;

        // Cleanup staging
        let _ = fs::remove_dir_all(&self.staging_dir);
        let _ = fs::create_dir_all(&self.staging_dir);

        Ok(InstallResult {
            previous_version: env!("CARGO_PKG_VERSION").to_string(),
            new_version: release.version().to_string(),
            backup_path,
        })
    }

    /// Download a release asset
    fn download_asset(&self, asset: &ReleaseAsset) -> Result<PathBuf, UpdateError> {
        let target_path = self.staging_dir.join(&asset.name);

        let response = crate::net::agent()
            .get(&asset.browser_download_url)
            .header("User-Agent", &crate::net::user_agent())
            .call()
            .map_err(|e| UpdateError::DownloadFailed(e.to_string()))?;

        // BufWriter so the ~8KB chunks ureq emits don't translate to
        // ~3,800 raw write(2) syscalls for a 30 MB tarball.
        let file = File::create(&target_path)
            .map_err(|e| UpdateError::DownloadFailed(format!("Cannot create file: {}", e)))?;
        let mut writer = std::io::BufWriter::with_capacity(64 * 1024, file);

        let mut body = response.into_body();
        let mut reader = body.as_reader();
        io::copy(&mut reader, &mut writer)
            .map_err(|e| UpdateError::DownloadFailed(format!("Download failed: {}", e)))?;
        writer
            .flush()
            .map_err(|e| UpdateError::DownloadFailed(format!("Flush failed: {}", e)))?;

        Ok(target_path)
    }

    /// Download checksums file
    fn download_checksums(&self, asset: &ReleaseAsset) -> Result<String, UpdateError> {
        let response = crate::net::agent()
            .get(&asset.browser_download_url)
            .header("User-Agent", &crate::net::user_agent())
            .call()
            .map_err(|e| UpdateError::DownloadFailed(e.to_string()))?;

        let mut body_content = String::new();
        let mut body = response.into_body();
        body.as_reader()
            .read_to_string(&mut body_content)
            .map_err(|e| UpdateError::DownloadFailed(e.to_string()))?;

        Ok(body_content)
    }

    /// Verify archive checksum against downloaded checksums
    fn verify_archive_checksum(
        &self,
        archive_path: &Path,
        archive_name: &str,
        checksums: &str,
    ) -> Result<(), UpdateError> {
        // Find the line with our archive
        let expected = checksums
            .lines()
            .find(|line| line.contains(archive_name))
            .and_then(|line| line.split_whitespace().next())
            .ok_or(UpdateError::ChecksumMismatch)?;

        // Calculate actual checksum
        let actual = calculate_file_checksum(archive_path)?;

        if actual.to_lowercase() != expected.to_lowercase() {
            return Err(UpdateError::ChecksumMismatch);
        }

        Ok(())
    }

    /// Extract binary from archive
    fn extract_binary(&self, archive_path: &Path) -> Result<PathBuf, UpdateError> {
        let archive_name = archive_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        let extract_dir = self.staging_dir.join("extract");
        fs::create_dir_all(&extract_dir)
            .map_err(|e| UpdateError::InstallationFailed(e.to_string()))?;

        if archive_name.ends_with(".tar.gz") || archive_name.ends_with(".tgz") {
            self.extract_tar_gz(archive_path, &extract_dir)?;
        } else if archive_name.ends_with(".zip") {
            self.extract_zip(archive_path, &extract_dir)?;
        } else {
            return Err(UpdateError::InstallationFailed(
                "Unknown archive format".to_string(),
            ));
        }

        // Find the jarvy binary in extracted files
        let binary_name = if cfg!(windows) { "jarvy.exe" } else { "jarvy" };
        self.find_binary(&extract_dir, binary_name)
    }

    /// Extract tar.gz archive.
    ///
    /// Uses the system `tar` with flags that prevent classic path-traversal
    /// vectors (`--no-same-owner`, no absolute paths, no symlink writes
    /// outside `dest`). Each entry's resolved path is also re-validated to
    /// be inside `dest` post-extract to defend against older tar versions
    /// that don't honor every flag.
    fn extract_tar_gz(&self, archive: &Path, dest: &Path) -> Result<(), UpdateError> {
        use std::process::Command;

        let dest_canon = fs::canonicalize(dest).map_err(|e| {
            UpdateError::InstallationFailed(format!("canonicalize destination: {e}"))
        })?;

        // Args that are accepted by both GNU tar and BSD tar:
        //   -x   extract
        //   -z   gzip
        //   -f   from file
        //   --no-same-owner   never restore archived uid/gid
        //   --no-same-permissions  never restore archived mode bits
        // Note: BSD tar interprets -P inverted compared to GNU; we instead
        // post-validate that no extracted path escaped `dest`.
        let status = Command::new("tar")
            .args([
                "-xzf",
                archive.to_string_lossy().as_ref(),
                "-C",
                dest_canon.to_string_lossy().as_ref(),
                "--no-same-owner",
                "--no-same-permissions",
            ])
            .status()
            .map_err(|e| {
                UpdateError::InstallationFailed(format!("tar extraction failed: {}", e))
            })?;

        if !status.success() {
            return Err(UpdateError::InstallationFailed(
                "tar extraction returned non-zero".to_string(),
            ));
        }

        // Walk the extracted tree and refuse to install if any entry resolved
        // outside the destination — catches symlink/PaxHeader escapes that
        // older GNU tar versions are vulnerable to.
        if let Err(e) = verify_no_tar_escape(&dest_canon) {
            return Err(UpdateError::InstallationFailed(format!(
                "tar archive contained a path that escaped {}: {}",
                dest_canon.display(),
                e
            )));
        }

        Ok(())
    }

    /// Extract zip archive
    fn extract_zip(&self, archive: &Path, dest: &Path) -> Result<(), UpdateError> {
        let file =
            File::open(archive).map_err(|e| UpdateError::InstallationFailed(e.to_string()))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| UpdateError::InstallationFailed(e.to_string()))?;

        archive
            .extract(dest)
            .map_err(|e| UpdateError::InstallationFailed(e.to_string()))?;

        Ok(())
    }

    /// Find binary in extracted directory
    fn find_binary(&self, dir: &Path, name: &str) -> Result<PathBuf, UpdateError> {
        // Try direct match first
        let direct = dir.join(name);
        if direct.exists() {
            return Ok(direct);
        }

        // Search recursively
        for entry in walkdir(dir).flatten() {
            if entry.file_name().to_string_lossy() == name {
                return Ok(entry.path().to_path_buf());
            }
        }

        Err(UpdateError::InstallationFailed(format!(
            "Binary {} not found in archive",
            name
        )))
    }

    /// Backup current binary
    fn backup_current(&self, current_exe: &Path) -> Result<PathBuf, UpdateError> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let backup_name = format!("jarvy-{}-{}", env!("CARGO_PKG_VERSION"), timestamp);
        let backup_path = self.backup_dir.join(backup_name);

        fs::copy(current_exe, &backup_path)
            .map_err(|e| UpdateError::InstallationFailed(format!("Backup failed: {}", e)))?;

        Ok(backup_path)
    }

    /// Replace current binary with new one
    fn replace_binary(&self, new_binary: &Path, target: &Path) -> Result<(), UpdateError> {
        // Make new binary executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(new_binary)
                .map_err(|e| UpdateError::InstallationFailed(e.to_string()))?
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(new_binary, perms)
                .map_err(|e| UpdateError::InstallationFailed(e.to_string()))?;
        }

        // Use self_update for atomic replacement
        self_update::Move::from_source(new_binary)
            .replace_using_temp(target)
            .to_dest(target)
            .map_err(|e| UpdateError::InstallationFailed(e.to_string()))?;

        Ok(())
    }

    /// Verify the new installation works
    fn verify_installation(&self, exe_path: &Path) -> Result<bool, UpdateError> {
        use std::process::Command;

        let output = Command::new(exe_path)
            .arg("--version")
            .output()
            .map_err(|e| UpdateError::InstallationFailed(e.to_string()))?;

        Ok(output.status.success())
    }

    /// Restore backup after failed installation
    fn restore_backup(&self, backup: &Path, target: &Path) -> Result<(), UpdateError> {
        fs::copy(backup, target)
            .map_err(|e| UpdateError::RollbackFailed(format!("Restore failed: {}", e)))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(target)
                .map_err(|e| UpdateError::RollbackFailed(e.to_string()))?
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(target, perms)
                .map_err(|e| UpdateError::RollbackFailed(e.to_string()))?;
        }

        Ok(())
    }
}

/// Walk every entry under `dest_canon` and verify the canonicalized path is
/// still contained in `dest_canon`. Refuses symlinks pointing outside the
/// extraction root. Errors describe the offending path.
fn verify_no_tar_escape(dest_canon: &Path) -> Result<(), String> {
    // Inline shallow recursive walk; we don't want a new dependency.
    fn walk(root: &Path, dir: &Path) -> Result<(), String> {
        let entries = fs::read_dir(dir).map_err(|e| format!("read_dir failed: {e}"))?;
        for entry in entries.flatten() {
            let path = entry.path();
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(e) => return Err(format!("metadata failed for {}: {e}", path.display())),
            };
            // For symlinks, canonicalize follows the link target; ensure the
            // target resolves inside `root`.
            let resolved = fs::canonicalize(&path)
                .map_err(|e| format!("canonicalize failed for {}: {e}", path.display()))?;
            if !resolved.starts_with(root) {
                return Err(format!(
                    "path {} resolves to {} which is outside {}",
                    path.display(),
                    resolved.display(),
                    root.display()
                ));
            }
            if metadata.is_dir() && !metadata.is_symlink() {
                walk(root, &path)?;
            }
        }
        Ok(())
    }
    walk(dest_canon, dest_canon)
}

impl Default for BinaryInstaller {
    fn default() -> Self {
        match Self::new() {
            Ok(installer) => installer,
            Err(e) => {
                eprintln!("Warning: failed to create BinaryInstaller: {e}");
                // Provide a fallback using temp directories
                Self {
                    backup_dir: std::env::temp_dir().join("jarvy-backup"),
                    staging_dir: std::env::temp_dir().join("jarvy-staging"),
                }
            }
        }
    }
}

/// Result of a successful installation
#[derive(Debug)]
pub struct InstallResult {
    pub previous_version: String,
    pub new_version: String,
    pub backup_path: PathBuf,
}

/// Calculate SHA256 checksum of a file
fn calculate_file_checksum(path: &Path) -> Result<String, UpdateError> {
    let mut file = File::open(path).map_err(|e| UpdateError::InstallationFailed(e.to_string()))?;

    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|e| UpdateError::InstallationFailed(e.to_string()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(hex::encode(hasher.finalize()))
}

/// Simple directory walker
fn walkdir(dir: &Path) -> impl Iterator<Item = io::Result<fs::DirEntry>> {
    let mut stack = vec![dir.to_path_buf()];
    std::iter::from_fn(move || {
        while let Some(current) = stack.pop() {
            if let Ok(entries) = fs::read_dir(&current) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        stack.push(entry.path());
                    } else {
                        return Some(Ok(entry));
                    }
                }
            }
        }
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_calculate_checksum() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"hello world").unwrap();
        drop(file);

        let checksum = calculate_file_checksum(&file_path).unwrap();
        // SHA256 of "hello world"
        assert_eq!(
            checksum,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_installer_creation() {
        // Just verify installer can be created
        let installer = BinaryInstaller::new();
        assert!(installer.is_ok());
    }
}
