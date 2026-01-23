//! Signature and checksum verification for updates
//!
//! Provides secure verification of downloaded binaries.

#![allow(dead_code)] // Public API for signature verification

use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Verify a file's SHA256 checksum
pub fn verify_checksum(file_path: &Path, expected: &str) -> Result<bool, VerifyError> {
    let actual = calculate_sha256(file_path)?;
    Ok(actual.to_lowercase() == expected.to_lowercase())
}

/// Calculate SHA256 hash of a file
pub fn calculate_sha256(file_path: &Path) -> Result<String, VerifyError> {
    let mut file = File::open(file_path).map_err(VerifyError::Io)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let n = file.read(&mut buffer).map_err(VerifyError::Io)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(hex::encode(hasher.finalize()))
}

/// Parse a checksums file (SHA256SUMS format)
/// Returns a list of (checksum, filename) tuples
pub fn parse_checksums(content: &str) -> Vec<(String, String)> {
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }

            // Format: "checksum  filename" or "checksum *filename"
            let mut parts = line.splitn(2, |c: char| c.is_whitespace());
            let checksum = parts.next()?.trim();
            let filename = parts.next()?.trim().trim_start_matches('*');

            if checksum.len() == 64 && !filename.is_empty() {
                Some((checksum.to_string(), filename.to_string()))
            } else {
                None
            }
        })
        .collect()
}

/// Find checksum for a specific file in checksums content
pub fn find_checksum(checksums_content: &str, filename: &str) -> Option<String> {
    let checksums = parse_checksums(checksums_content);
    checksums
        .into_iter()
        .find(|(_, name)| name == filename || name.ends_with(filename))
        .map(|(sum, _)| sum)
}

/// Errors during verification
#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Checksum mismatch")]
    ChecksumMismatch,

    #[error("Checksum not found for file: {0}")]
    ChecksumNotFound(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_calculate_sha256() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"hello world").unwrap();
        drop(file);

        let hash = calculate_sha256(&file_path).unwrap();
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_verify_checksum() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"hello world").unwrap();
        drop(file);

        let valid = verify_checksum(
            &file_path,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
        )
        .unwrap();
        assert!(valid);

        let invalid = verify_checksum(
            &file_path,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();
        assert!(!invalid);
    }

    #[test]
    fn test_parse_checksums() {
        // SHA256 hashes are 64 hex characters
        let content = r#"
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  jarvy-darwin-aarch64.tar.gz
bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb  jarvy-linux-x86_64.tar.gz
# Comment line
cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc *jarvy-windows-x86_64.zip
"#;

        let checksums = parse_checksums(content);
        assert_eq!(checksums.len(), 3);
        assert_eq!(
            checksums[0].0,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(checksums[0].1, "jarvy-darwin-aarch64.tar.gz");
        assert_eq!(checksums[2].1, "jarvy-windows-x86_64.zip");
    }

    #[test]
    fn test_find_checksum() {
        // SHA256 hashes are 64 hex characters
        let content = r#"
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  jarvy-darwin-aarch64.tar.gz
bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb  jarvy-linux-x86_64.tar.gz
"#;

        let found = find_checksum(content, "jarvy-darwin-aarch64.tar.gz");
        assert_eq!(
            found,
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string())
        );

        let not_found = find_checksum(content, "nonexistent.tar.gz");
        assert_eq!(not_found, None);
    }
}
