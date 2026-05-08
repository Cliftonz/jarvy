//! Common utilities for package handlers
//!
//! Provides shared functionality for running package manager commands
//! and handling errors across different package ecosystems.

use std::path::Path;
use std::process::Command;
use thiserror::Error;

/// Errors that can occur during package installation
#[derive(Debug, Error)]
pub enum PackageError {
    #[error("Package manager not installed: {0}")]
    PackageManagerNotInstalled(String),

    #[error("Lock file not found: {0}")]
    LockfileNotFound(String),

    #[error("Command failed: {0}")]
    CommandFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Virtual environment creation failed: {0}")]
    VenvCreationFailed(String),

    #[error("Package installation failed: {0}")]
    #[allow(dead_code)] // Reserved for future use
    InstallFailed(String),

    /// A `[npm]/[pip]/[cargo]` package name or version was rejected because
    /// it would inject CLI flags or a non-package URL into the underlying
    /// package manager's argument list. Examples:
    ///
    ///     "--registry=http://attacker"
    ///     "--git"          (cargo flag)
    ///     "../../etc/passwd"
    ///     "git+https://attacker/x.git"
    ///
    /// All of these are passed positionally to `npm install` / `cargo install`
    /// / `pip install` and would normally be honored as flags or URL deps.
    #[error("Refused unsafe package spec '{0}': {1}")]
    RefusedUnsafeSpec(String, String),
}

/// Validate a package name (`[npm]/[pip]/[cargo]` key) before passing it as
/// a positional arg. Rejects:
/// - empty
/// - leading `-` (collides with CLI flag namespace)
/// - URL/scheme prefixes (`git+`, `http://`, `https://`, `file:`, `npm:`,
///   `./`, `../`) — these are direct-URL deps that bypass the registry
/// - chars outside the safe set `[A-Za-z0-9._@/+:~-]` (npm scoped names use
///   `@scope/name`, pip extras use `[extra]` but those go in the version
///   spec, not the name; cargo crate names are `[A-Za-z0-9_-]+`)
///
/// `purpose` is included in the error message and `tracing::warn!` event for
/// support.
pub fn validate_package_name(name: &str, purpose: &'static str) -> Result<(), PackageError> {
    if name.is_empty() {
        return Err(PackageError::RefusedUnsafeSpec(
            name.to_string(),
            format!("{purpose} package name is empty"),
        ));
    }
    if name.starts_with('-') {
        tracing::warn!(
            event = "packages.refused_flag_like_name",
            purpose = %purpose,
            name = %name,
        );
        return Err(PackageError::RefusedUnsafeSpec(
            name.to_string(),
            format!("{purpose} package name starts with `-`; would be interpreted as a CLI flag"),
        ));
    }
    const SCHEMES: &[&str] = &["git+", "http://", "https://", "file:", "npm:", "./", "../"];
    for scheme in SCHEMES {
        if name.starts_with(scheme) {
            tracing::warn!(
                event = "packages.refused_url_scheme",
                purpose = %purpose,
                scheme = %scheme,
                name = %name,
            );
            return Err(PackageError::RefusedUnsafeSpec(
                name.to_string(),
                format!(
                    "{purpose} package spec uses scheme `{scheme}` which bypasses the registry; \
                     direct-URL deps are not accepted from jarvy.toml"
                ),
            ));
        }
    }
    if !name.chars().all(|c| {
        c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '@' | '/' | '+' | ':' | '~' | '-')
    }) {
        tracing::warn!(
            event = "packages.refused_unsafe_chars",
            purpose = %purpose,
            name = %name,
        );
        return Err(PackageError::RefusedUnsafeSpec(
            name.to_string(),
            format!("{purpose} package name contains characters outside the safe set"),
        ));
    }
    Ok(())
}

/// Validate a version specifier (`{name} = "<version>"` value). Same shape as
/// the name guard but tolerates additional version characters. Rejects empty,
/// leading `-`, and URL schemes.
pub fn validate_package_version(version: &str, purpose: &'static str) -> Result<(), PackageError> {
    if version.is_empty() {
        return Err(PackageError::RefusedUnsafeSpec(
            version.to_string(),
            format!("{purpose} version is empty"),
        ));
    }
    if version.starts_with('-') {
        return Err(PackageError::RefusedUnsafeSpec(
            version.to_string(),
            format!("{purpose} version starts with `-`; would be interpreted as a CLI flag"),
        ));
    }
    const SCHEMES: &[&str] = &["git+", "http://", "https://", "file:", "npm:"];
    for scheme in SCHEMES {
        if version.starts_with(scheme) {
            return Err(PackageError::RefusedUnsafeSpec(
                version.to_string(),
                format!("{purpose} version uses scheme `{scheme}` which bypasses the registry"),
            ));
        }
    }
    Ok(())
}

/// Run a package manager command with the given arguments
pub fn run_package_command(
    cmd: &str,
    args: &[&str],
    working_dir: &Path,
) -> Result<(), PackageError> {
    let display_cmd = format!("{} {}", cmd, args.join(" "));
    println!("    Running: {}", display_cmd);

    let status = Command::new(cmd)
        .args(args)
        .current_dir(working_dir)
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                PackageError::PackageManagerNotInstalled(cmd.to_string())
            } else {
                PackageError::Io(e)
            }
        })?;

    if !status.success() {
        return Err(PackageError::CommandFailed(format!(
            "'{}' exited with status {}",
            display_cmd,
            status.code().unwrap_or(-1)
        )));
    }

    Ok(())
}

/// Check if a command is available in PATH
pub fn command_exists(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_exists() {
        // These commands should exist on most systems
        assert!(command_exists("echo") || command_exists("cmd"));
    }

    #[test]
    fn test_command_not_exists() {
        assert!(!command_exists(
            "this_command_definitely_does_not_exist_12345"
        ));
    }

    #[test]
    fn validate_name_accepts_normal_packages() {
        validate_package_name("typescript", "[npm]").unwrap();
        validate_package_name("@types/node", "[npm]").unwrap();
        validate_package_name("cargo-watch", "[cargo]").unwrap();
        validate_package_name("pytest", "[pip]").unwrap();
        validate_package_name("django-allauth", "[pip]").unwrap();
        validate_package_name("requests", "[pip]").unwrap();
    }

    #[test]
    fn validate_name_rejects_flag_like() {
        for hostile in [
            "--registry=http://attacker",
            "--git",
            "--root",
            "-y",
            "-e/etc/passwd",
        ] {
            let err = validate_package_name(hostile, "[npm]").unwrap_err();
            match err {
                PackageError::RefusedUnsafeSpec(_, _) => {}
                other => panic!("expected RefusedUnsafeSpec for {hostile:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn validate_name_rejects_url_schemes() {
        for hostile in [
            "git+https://attacker/x.git",
            "https://attacker/x",
            "http://attacker/x",
            "file:///etc/passwd",
            "npm:@scope/foo",
            "./local-evil",
            "../escape",
        ] {
            assert!(matches!(
                validate_package_name(hostile, "[cargo]"),
                Err(PackageError::RefusedUnsafeSpec(_, _))
            ));
        }
    }

    #[test]
    fn validate_name_rejects_unsafe_chars() {
        for hostile in ["pkg;rm -rf /", "pkg|other", "pkg&", "pkg`evil`", "pkg$VAR"] {
            assert!(matches!(
                validate_package_name(hostile, "[npm]"),
                Err(PackageError::RefusedUnsafeSpec(_, _))
            ));
        }
    }

    #[test]
    fn validate_name_rejects_empty() {
        assert!(matches!(
            validate_package_name("", "[npm]"),
            Err(PackageError::RefusedUnsafeSpec(_, _))
        ));
    }

    #[test]
    fn validate_version_accepts_normal_specs() {
        validate_package_version("1.0", "[npm]").unwrap();
        validate_package_version("^1.0.0", "[npm]").unwrap();
        validate_package_version("~1.0", "[npm]").unwrap();
        validate_package_version(">=2.0", "[pip]").unwrap();
        validate_package_version("latest", "[cargo]").unwrap();
        validate_package_version("0.9.0-beta.1", "[cargo]").unwrap();
    }

    #[test]
    fn validate_version_rejects_flags_and_urls() {
        for hostile in [
            "--registry=http://x",
            "-r",
            "git+https://x",
            "https://attacker",
        ] {
            assert!(matches!(
                validate_package_version(hostile, "[cargo]"),
                Err(PackageError::RefusedUnsafeSpec(_, _))
            ));
        }
    }
}
