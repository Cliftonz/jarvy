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

/// Anchored Fulcio cert-identity regex. Anchors the host (`github.com`),
/// the repo path, the workflow file, and the tag-ref so a Fulcio cert whose
/// Subject merely *contains* `github.com/bearbinary/jarvy` (e.g. an attacker
/// fork's workflow URL with that substring) is rejected.
pub(crate) const COSIGN_CERT_IDENTITY_REGEX: &str = concat!(
    r"^https://github\.com/bearbinary/jarvy/\.github/workflows/[^@]+@",
    r"refs/tags/v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z\.\-]+)?$",
);

/// Result of a Sigstore verification attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureOutcome {
    /// `cosign verify-blob` succeeded.
    Verified,
    /// `cosign` is not on PATH.
    CosignMissing,
    /// `.sig` or `.pem` files were not found alongside the artifact.
    SignatureFilesMissing,
    /// Cosign ran and rejected the artifact.
    Rejected(String),
}

/// Verify a file's Sigstore signature using cosign with the canonical
/// Jarvy-release identity. Wrapper over
/// [`verify_sigstore_signature_with_identity`] that pins the cosign
/// identity-regexp + OIDC issuer to the Jarvy release workflow.
///
/// Returns a `SignatureOutcome` describing what happened — fail-OPEN
/// handling (treating cosign-missing or sig-missing as success) is **the
/// caller's decision**. The installer fails closed unless explicit
/// override is granted.
pub fn verify_sigstore_signature(file_path: &Path) -> Result<SignatureOutcome, VerifyError> {
    verify_sigstore_signature_with_identity(
        file_path,
        COSIGN_CERT_IDENTITY_REGEX,
        "https://token.actions.githubusercontent.com",
    )
}

/// Verify a file's Sigstore signature against a caller-supplied
/// `identity_regexp` and `oidc_issuer`. Used by both the canonical
/// Jarvy-release verification ([`verify_sigstore_signature`], which pins
/// to the release workflow) and by `registry_remote::sync` (which pins
/// to whatever registry repo the user subscribed to).
///
/// Identity-regexp **MUST** be fully anchored (`^…$`). Callers that load
/// the regex from user config should refuse missing anchors before
/// reaching this function.
pub fn verify_sigstore_signature_with_identity(
    file_path: &Path,
    identity_regexp: &str,
    oidc_issuer: &str,
) -> Result<SignatureOutcome, VerifyError> {
    if !cosign_on_path() {
        tracing::warn!(
            event = "signature.skipped",
            reason = "cosign_missing",
            file = %file_path.display(),
        );
        return Ok(SignatureOutcome::CosignMissing);
    }

    let sig_path = file_path.with_extension(format!(
        "{}.sig",
        file_path.extension().unwrap_or_default().to_string_lossy()
    ));
    let cert_path = file_path.with_extension(format!(
        "{}.pem",
        file_path.extension().unwrap_or_default().to_string_lossy()
    ));

    if !sig_path.exists() || !cert_path.exists() {
        tracing::warn!(
            event = "signature.skipped",
            reason = "sig_files_missing",
            file = %file_path.display(),
        );
        return Ok(SignatureOutcome::SignatureFilesMissing);
    }

    use std::process::Command;
    let output = Command::new("cosign")
        .args([
            "verify-blob",
            "--signature",
            &sig_path.to_string_lossy(),
            "--certificate",
            &cert_path.to_string_lossy(),
            "--certificate-identity-regexp",
            identity_regexp,
            "--certificate-oidc-issuer",
            oidc_issuer,
        ])
        .arg(file_path)
        .output()
        .map_err(VerifyError::Io)?;

    if output.status.success() {
        tracing::info!(
            event = "signature.verified",
            file = %file_path.display(),
        );
        Ok(SignatureOutcome::Verified)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        tracing::error!(
            event = "signature.failed",
            file = %file_path.display(),
            error = %stderr,
        );
        Ok(SignatureOutcome::Rejected(stderr))
    }
}

/// `which::which`-style PATH lookup for cosign, cached once per process.
/// Replaces the previous "spawn cosign --version as a presence probe"
/// pattern (Perf F3) — the spawn cost is 80–200 ms cold-start per call;
/// a PATH lookup is microseconds.
fn cosign_on_path() -> bool {
    use std::sync::OnceLock;
    static FOUND: OnceLock<bool> = OnceLock::new();
    *FOUND.get_or_init(|| {
        // Walk $PATH manually to avoid a `which` dep. The cosign binary
        // is named `cosign` on every supported platform; on Windows the
        // .exe extension is checked too.
        let Some(path_var) = std::env::var_os("PATH") else {
            return false;
        };
        let names: &[&str] = if cfg!(windows) {
            &["cosign.exe", "cosign"]
        } else {
            &["cosign"]
        };
        for dir in std::env::split_paths(&path_var) {
            for name in names {
                if dir.join(name).is_file() {
                    return true;
                }
            }
        }
        false
    })
}

/// Decide whether a `SignatureOutcome` should permit installation to proceed.
///
/// `allow_unsigned` should be set only when the operator has explicitly opted
/// into unsigned updates (CLI `--allow-unsigned` flag or
/// `JARVY_ALLOW_UNSIGNED_UPDATE=1` env). Default (`false`) is fail-closed.
pub fn signature_outcome_is_acceptable(
    outcome: &SignatureOutcome,
    allow_unsigned: bool,
) -> Result<(), String> {
    match outcome {
        SignatureOutcome::Verified => Ok(()),
        SignatureOutcome::CosignMissing => {
            if allow_unsigned {
                Ok(())
            } else {
                Err(
                    "cosign is not installed; install it (https://docs.sigstore.dev/cosign/) \
                     or re-run with --allow-unsigned to accept supply-chain risk"
                        .to_string(),
                )
            }
        }
        SignatureOutcome::SignatureFilesMissing => {
            // Tightened by security review F-4. `--allow-unsigned` should
            // ONLY rubber-stamp `CosignMissing` (an environment problem on
            // the user's machine). `SignatureFilesMissing` means the
            // *release* didn't include signatures — accepting that is
            // accepting an attacker-tampered release. Always fail.
            Err(
                "release does not include .sig/.pem files; refusing to install \
                 unsigned binary. This is a release-side problem, not a local one — \
                 contact the release authors rather than overriding."
                    .to_string(),
            )
        }
        SignatureOutcome::Rejected(stderr) => Err(format!(
            "Sigstore verification rejected the artifact: {stderr}"
        )),
    }
}

/// Read `JARVY_ALLOW_UNSIGNED_UPDATE` and treat exactly `1`, `true`, or
/// `yes` (case-insensitive) as "permit unsigned updates."
///
/// Previous logic treated any non-empty / non-`0` / non-`false` / non-`no`
/// value as truthy, so typos like `disable` or `N` would inadvertently
/// permit unsigned updates. Strict allowlist closes that.
pub fn unsigned_override_from_env() -> bool {
    match std::env::var("JARVY_ALLOW_UNSIGNED_UPDATE") {
        Ok(v) => {
            let t = v.trim().to_ascii_lowercase();
            matches!(t.as_str(), "1" | "true" | "yes")
        }
        Err(_) => false,
    }
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

    #[error("Signature verification failed: {0}")]
    SignatureInvalid(String),
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
    fn cert_identity_regex_is_anchored() {
        let re = regex::Regex::new(COSIGN_CERT_IDENTITY_REGEX).expect("valid regex");
        // Legitimate identity from a release-tag workflow.
        assert!(re.is_match(
            "https://github.com/bearbinary/jarvy/.github/workflows/release.yml@refs/tags/v1.2.3"
        ));
        assert!(re.is_match(
            "https://github.com/bearbinary/jarvy/.github/workflows/release.yml@refs/tags/v1.2.3-rc.1"
        ));
        // Substring attack: attacker repo's path happens to contain the substring.
        assert!(!re.is_match(
            "https://github.com/attacker/repo/.github/workflows/foo.yml@refs/heads/main\
             github.com/bearbinary/jarvy"
        ));
        // Non-tag ref must be rejected (workflow-on-branch).
        assert!(!re.is_match(
            "https://github.com/bearbinary/jarvy/.github/workflows/release.yml@refs/heads/main"
        ));
        // Wrong host.
        assert!(!re.is_match(
            "https://gitlab.com/bearbinary/jarvy/.github/workflows/release.yml@refs/tags/v1.0.0"
        ));
    }

    #[test]
    fn cosign_missing_is_not_acceptable_by_default() {
        let outcome = SignatureOutcome::CosignMissing;
        assert!(signature_outcome_is_acceptable(&outcome, false).is_err());
    }

    #[test]
    fn cosign_missing_acceptable_with_override() {
        let outcome = SignatureOutcome::CosignMissing;
        assert!(signature_outcome_is_acceptable(&outcome, true).is_ok());
    }

    #[test]
    fn sig_files_missing_is_not_acceptable_by_default() {
        let outcome = SignatureOutcome::SignatureFilesMissing;
        assert!(signature_outcome_is_acceptable(&outcome, false).is_err());
    }

    #[test]
    fn sig_files_missing_unacceptable_even_with_override() {
        // Round-2 P0 regression guard: `--allow-unsigned` must NOT
        // rubber-stamp `SignatureFilesMissing` (a release-side problem
        // that's indistinguishable from a tampered release).
        // `CosignMissing` (an environment problem on the user's machine)
        // IS rubber-stampable; that distinction is the whole point of the
        // tightened policy. A revert to `if allow_unsigned { Ok(()) }`
        // here would compile and pass every other test in this module —
        // this is the test that catches it.
        let outcome = SignatureOutcome::SignatureFilesMissing;
        let err = signature_outcome_is_acceptable(&outcome, true)
            .expect_err("SignatureFilesMissing must always be Err, even with allow_unsigned=true");
        // Wording check so a relaxation that drops the "release-side"
        // explanation also surfaces.
        assert!(
            err.contains("release-side") || err.contains("refus"),
            "error message must indicate this is a release-side problem; got {err:?}"
        );
    }

    #[test]
    fn rejected_outcome_never_acceptable() {
        let outcome = SignatureOutcome::Rejected("bad cert".into());
        assert!(signature_outcome_is_acceptable(&outcome, true).is_err());
        assert!(signature_outcome_is_acceptable(&outcome, false).is_err());
    }

    #[test]
    fn verified_outcome_always_acceptable() {
        let outcome = SignatureOutcome::Verified;
        assert!(signature_outcome_is_acceptable(&outcome, false).is_ok());
        assert!(signature_outcome_is_acceptable(&outcome, true).is_ok());
    }

    #[test]
    #[serial_test::serial(jarvy_allow_unsigned_env)]
    fn unsigned_override_env_parsing() {
        // Round-2 QA B7 / item 16: serialized via #[serial] so a
        // parallel test reading JARVY_ALLOW_UNSIGNED_UPDATE during
        // BinaryInstaller::install_with_options doesn't observe
        // arbitrary mid-flight values from this test's mutations.
        let key = "JARVY_ALLOW_UNSIGNED_UPDATE";
        let prev = std::env::var(key).ok();

        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var(key);
        }
        assert!(!unsigned_override_from_env());

        for truthy in ["1", "true", "yes", "TRUE", "Yes"] {
            #[allow(unsafe_code)]
            unsafe {
                std::env::set_var(key, truthy);
            }
            assert!(
                unsigned_override_from_env(),
                "expected truthy for value {truthy:?}"
            );
        }

        // Strict allowlist (security review F-22): anything outside the
        // explicit truthy set — including typos like "Y", "disable",
        // "n/a" — must NOT permit unsigned updates. Previously these
        // landed in the truthy branch via a "non-empty / non-0 / non-false"
        // negative filter, which was too permissive.
        for falsy in ["0", "false", "no", "", "Y", "disable", "n/a", "off"] {
            #[allow(unsafe_code)]
            unsafe {
                std::env::set_var(key, falsy);
            }
            assert!(
                !unsigned_override_from_env(),
                "expected falsy for value {falsy:?}"
            );
        }

        // Restore.
        match prev {
            Some(v) => {
                #[allow(unsafe_code)]
                unsafe {
                    std::env::set_var(key, v);
                }
            }
            None => {
                #[allow(unsafe_code)]
                unsafe {
                    std::env::remove_var(key);
                }
            }
        }
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
