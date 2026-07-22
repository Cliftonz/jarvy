//! Git commit signing configuration
//!
//! Handles SSH and GPG key detection and configuration for commit signing.

use std::path::Path;
use std::process::Command;

use super::config::SigningFormat;

/// Detect the signing format from a key path or ID
#[allow(dead_code)] // Public API for signing configuration
pub fn detect_signing_format(key: &str) -> SigningFormat {
    // SSH keys typically end with .pub
    if key.ends_with(".pub") {
        return SigningFormat::Ssh;
    }

    // Check if it looks like a file path
    let expanded = shellexpand::tilde(key);
    let path = Path::new(expanded.as_ref());

    if path.exists() && path.is_file() {
        // Read first line to detect format
        if let Ok(content) = std::fs::read_to_string(path)
            && (content.starts_with("ssh-") || content.starts_with("ecdsa-"))
        {
            return SigningFormat::Ssh;
        }
    }

    // Default to GPG for key IDs
    SigningFormat::Gpg
}

/// Validate that a signing key exists and is readable
#[allow(dead_code)] // Public API for key validation
pub fn validate_signing_key(key: &str, format: SigningFormat) -> Result<(), String> {
    match format {
        SigningFormat::Ssh => {
            let expanded = shellexpand::tilde(key);
            let path = Path::new(expanded.as_ref());

            if !path.exists() {
                return Err(format!("SSH key not found: {key}"));
            }

            if !path.is_file() {
                return Err(format!("SSH key is not a file: {key}"));
            }

            Ok(())
        }
        SigningFormat::Gpg => {
            // Check if GPG key exists using gpg command
            let output = Command::new("gpg")
                .args(["--list-secret-keys", key])
                .output();

            match output {
                Ok(o) if o.status.success() => Ok(()),
                Ok(_) => Err(format!("GPG key not found: {key}")),
                Err(e) => Err(format!("Failed to check GPG key: {e}")),
            }
        }
    }
}

/// Get SSH allowed signers file path
#[allow(dead_code)] // Public API for SSH signing setup
pub fn ssh_allowed_signers_path() -> String {
    let home = dirs::home_dir().map(|p| p.to_string_lossy().to_string());
    match home {
        Some(h) => format!("{h}/.ssh/allowed_signers"),
        None => "~/.ssh/allowed_signers".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_signing_format_ssh() {
        assert_eq!(
            detect_signing_format("~/.ssh/id_ed25519.pub"),
            SigningFormat::Ssh
        );
        assert_eq!(
            detect_signing_format("/home/user/.ssh/id_rsa.pub"),
            SigningFormat::Ssh
        );
    }

    #[test]
    fn test_detect_signing_format_gpg() {
        assert_eq!(detect_signing_format("ABC123DEF456"), SigningFormat::Gpg);
        assert_eq!(
            detect_signing_format("user@example.com"),
            SigningFormat::Gpg
        );
    }

    #[test]
    fn test_ssh_allowed_signers_path() {
        let path = ssh_allowed_signers_path();
        assert!(path.ends_with("allowed_signers"));
    }
}
