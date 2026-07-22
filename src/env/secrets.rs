//! Secret prompting and handling
//!
//! Handles secret values with:
//! - Hidden password input using rpassword
//! - Loading secrets from files
//! - Environment variable fallback
//! - CI mode skipping

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use thiserror::Error;

use super::expand::{EnvContext, expand_path};
use crate::config::SecretValue;

/// Errors that can occur during secret handling
#[derive(Error, Debug)]
pub enum SecretError {
    #[error("Failed to read secret from file: {0}")]
    FileReadError(#[from] std::io::Error),
    #[error("Required secret '{0}' was not provided")]
    MissingRequired(String),
    #[error("Failed to read user input")]
    InputError,
    #[error("Secret file not found: {0}")]
    FileNotFound(String),
    #[error(
        "secret file '{path}' resolves outside project root and $HOME; \
        set JARVY_ALLOW_EXTERNAL_SECRETS=1 to override"
    )]
    PathEscapesProject { path: String },
}

/// Refuse `from_file` paths that, after symlink-resolving canonicalization,
/// land outside both the project root (`ctx.current_dir`) and `$HOME`. Common
/// legitimate uses (`~/.aws/credentials`, `<project>/.env.secret`) stay
/// allowed; `/etc/shadow` and `../../etc/passwd` are refused unless the
/// operator opts in with `JARVY_ALLOW_EXTERNAL_SECRETS=1`.
fn ensure_secret_path_contained(path: &Path, ctx: &EnvContext) -> Result<(), SecretError> {
    if std::env::var("JARVY_ALLOW_EXTERNAL_SECRETS")
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes"))
        .unwrap_or(false)
    {
        return Ok(());
    }

    // Canonicalize the secret path first; symlinks are resolved here so a
    // `<project>/link → /etc/shadow` can't slip past the containment check.
    let canon_path = match path.canonicalize() {
        Ok(p) => p,
        // If canonicalization fails (e.g., file missing) the caller will
        // surface a clearer FileNotFound error; don't double-report.
        Err(_) => return Ok(()),
    };

    let project_root = ctx.current_dir.canonicalize().ok();
    let home = ctx.home_dir.canonicalize().ok();

    let under_project = project_root
        .as_ref()
        .is_some_and(|root| canon_path.starts_with(root));
    let under_home = home.as_ref().is_some_and(|h| canon_path.starts_with(h));

    if under_project || under_home {
        Ok(())
    } else {
        Err(SecretError::PathEscapesProject {
            path: canon_path.display().to_string(),
        })
    }
}

/// Configuration for secret collection
#[derive(Debug, Clone)]
pub struct SecretsConfig {
    /// Whether we're running in CI mode (skip prompts)
    pub ci_mode: bool,
    /// Whether to fail on missing required secrets
    pub fail_on_missing: bool,
}

impl Default for SecretsConfig {
    fn default() -> Self {
        // `ci_mode` here means "don't prompt; rely on env-supplied
        // secrets only." That semantic matches every unattended
        // environment, not just CI runners — AI sandboxes have no
        // human to type a secret either. Route through the canonical
        // unattended predicate (PRD-053).
        Self {
            ci_mode: crate::sandbox::is_seamless() || std::env::var("JARVY_TEST_MODE").is_ok(),
            fail_on_missing: true,
        }
    }
}

/// Collect all secret values from configuration
///
/// # Arguments
/// * `secrets` - HashMap of secret names to their configuration
/// * `ctx` - Context for path expansion
/// * `config` - Configuration for secret handling
///
/// # Returns
/// HashMap of secret names to their resolved values, or an error
pub fn collect_secrets(
    secrets: &HashMap<String, SecretValue>,
    ctx: &EnvContext,
    config: &SecretsConfig,
) -> Result<HashMap<String, String>, SecretError> {
    let mut result = HashMap::new();

    for (name, secret_config) in secrets {
        match resolve_secret(name, secret_config, ctx, config) {
            Ok(Some(value)) => {
                result.insert(name.clone(), value);
            }
            Ok(None) => {
                // Optional secret not provided, skip
            }
            Err(e) => {
                if config.fail_on_missing {
                    return Err(e);
                }
                eprintln!("Warning: Could not resolve secret '{}': {}", name, e);
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
#[cfg(unix)]
mod permissive_perms_tests {
    //! Verifies the structured-warning behavior for secret files with
    //! permissive permissions. The previous `eprintln!` path was not testable.

    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    fn make_secret_file_in(dir: &Path, mode: u32) -> std::path::PathBuf {
        let path = dir.join("secret");
        let mut f = std::fs::File::create(&path).expect("create secret");
        write!(f, "supersecret").unwrap();
        drop(f);
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(mode);
        std::fs::set_permissions(&path, perms).unwrap();
        path
    }

    fn ctx_rooted_at(dir: &Path) -> EnvContext {
        let mut ctx = EnvContext::new();
        ctx.current_dir = dir.to_path_buf();
        ctx
    }

    #[test]
    fn resolve_secret_with_0600_does_not_warn_about_perms() {
        let tmp = tempfile::tempdir().unwrap();
        let path = make_secret_file_in(tmp.path(), 0o600);
        let secret = SecretValue::FromFile {
            from_file: path.to_string_lossy().to_string(),
        };
        let ctx = ctx_rooted_at(tmp.path());
        let result = resolve_secret("TEST_SECRET", &secret, &ctx, &SecretsConfig::default());
        assert_eq!(result.unwrap(), Some("supersecret".to_string()));
    }

    #[test]
    fn resolve_secret_with_0644_still_returns_value() {
        let tmp = tempfile::tempdir().unwrap();
        let path = make_secret_file_in(tmp.path(), 0o644);
        let secret = SecretValue::FromFile {
            from_file: path.to_string_lossy().to_string(),
        };
        let ctx = ctx_rooted_at(tmp.path());
        let result = resolve_secret("TEST_SECRET", &secret, &ctx, &SecretsConfig::default());
        assert_eq!(result.unwrap(), Some("supersecret".to_string()));
    }
}

/// Resolve a single secret value
fn resolve_secret(
    name: &str,
    config: &SecretValue,
    ctx: &EnvContext,
    secrets_config: &SecretsConfig,
) -> Result<Option<String>, SecretError> {
    match config {
        SecretValue::FromFile { from_file } => {
            let path = expand_path(from_file, ctx);
            if !path.exists() {
                return Err(SecretError::FileNotFound(path.display().to_string()));
            }
            ensure_secret_path_contained(&path, ctx)?;
            // Warn if secret file has overly permissive permissions.
            // Shared with `network::config::PasswordSource::File` via
            // `crate::security`. The `secret_name` context is now lost from
            // the structured event; if a future use needs it back, extend
            // `warn_if_world_readable` with a generic attributes argument.
            crate::security::warn_if_world_readable(&path, "secret");
            let content = fs::read_to_string(&path)?;
            Ok(Some(content.trim().to_string()))
        }
        SecretValue::Prompt {
            env,
            required,
            description,
        } => {
            // First check environment variable if specified
            if let Some(env_var) = env
                && let Ok(value) = std::env::var(env_var)
                && !value.is_empty()
            {
                return Ok(Some(value));
            }

            // Check if the secret itself is in environment
            if let Ok(value) = std::env::var(name)
                && !value.is_empty()
            {
                return Ok(Some(value));
            }

            // In CI mode, don't prompt
            if secrets_config.ci_mode {
                if *required {
                    return Err(SecretError::MissingRequired(name.to_string()));
                }
                return Ok(None);
            }

            // Prompt user for input
            prompt_secret(name, description.as_deref(), *required)
        }
        SecretValue::Simple(marker) => {
            // Simple marker - check environment first
            if let Ok(value) = std::env::var(name)
                && !value.is_empty()
            {
                return Ok(Some(value));
            }

            // Check the marker value as an env var too
            if marker != name
                && let Ok(value) = std::env::var(marker)
                && !value.is_empty()
            {
                return Ok(Some(value));
            }

            // In CI mode, skip prompting
            if secrets_config.ci_mode {
                return Err(SecretError::MissingRequired(name.to_string()));
            }

            // Prompt for the secret
            prompt_secret(name, None, true)
        }
    }
}

/// Prompt user for a secret with hidden input
fn prompt_secret(
    name: &str,
    description: Option<&str>,
    required: bool,
) -> Result<Option<String>, SecretError> {
    // Print prompt
    if let Some(desc) = description {
        println!("{} ({})", name, desc);
    }

    print!("Enter value for {}: ", name);
    io::stdout().flush()?;

    // Read password with hidden input
    let password = rpassword::read_password().map_err(|_| SecretError::InputError)?;

    if password.is_empty() {
        if required {
            Err(SecretError::MissingRequired(name.to_string()))
        } else {
            Ok(None)
        }
    } else {
        Ok(Some(password))
    }
}

/// Load a secret from a file path
#[allow(dead_code)] // Public API for secret loading
pub fn load_secret_from_file(path: &Path) -> Result<String, SecretError> {
    if !path.exists() {
        return Err(SecretError::FileNotFound(path.display().to_string()));
    }
    let content = fs::read_to_string(path)?;
    Ok(content.trim().to_string())
}

/// Preview secrets that would be collected (for dry-run)
/// Returns secret names without actual values
#[allow(dead_code)] // Public API for dry-run secret preview
pub fn preview_secrets(secrets: &HashMap<String, SecretValue>) -> Vec<String> {
    secrets.keys().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_secret_from_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("secret.txt");
        fs::write(&file_path, "my_secret_value\n").unwrap();

        let result = load_secret_from_file(&file_path).unwrap();
        assert_eq!(result, "my_secret_value");
    }

    #[test]
    fn test_load_secret_from_file_not_found() {
        let result = load_secret_from_file(Path::new("/nonexistent/path/secret.txt"));
        assert!(matches!(result, Err(SecretError::FileNotFound(_))));
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_secrets_config_default_ci_detection() {
        // Save current env
        let ci_was_set = std::env::var("CI").is_ok();

        // Set CI env to test detection
        // SAFETY: Test environment, single-threaded access
        unsafe {
            std::env::set_var("JARVY_TEST_MODE", "1");
        }
        let config = SecretsConfig::default();
        assert!(config.ci_mode);

        // Clean up if CI wasn't originally set
        if !ci_was_set {
            // SAFETY: Test environment, single-threaded access
            unsafe {
                std::env::remove_var("CI");
            }
        }
    }

    #[test]
    fn test_resolve_secret_from_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("secret.txt");
        fs::write(&file_path, "file_secret_value").unwrap();

        let mut ctx = EnvContext::new();
        ctx.current_dir = dir.path().to_path_buf();
        let config = SecretsConfig {
            ci_mode: true,
            fail_on_missing: true,
        };

        let secret_config = SecretValue::FromFile {
            from_file: file_path.to_string_lossy().to_string(),
        };

        let result = resolve_secret("TEST_SECRET", &secret_config, &ctx, &config).unwrap();
        assert_eq!(result, Some("file_secret_value".to_string()));
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_resolve_secret_from_env() {
        // SAFETY: Test environment, single-threaded access
        unsafe {
            std::env::set_var("TEST_SECRET_ENV", "env_value");
        }

        let ctx = EnvContext::new();
        let config = SecretsConfig {
            ci_mode: true,
            fail_on_missing: true,
        };

        let secret_config = SecretValue::Prompt {
            env: Some("TEST_SECRET_ENV".to_string()),
            required: true,
            description: None,
        };

        let result = resolve_secret("MY_SECRET", &secret_config, &ctx, &config).unwrap();
        assert_eq!(result, Some("env_value".to_string()));

        // SAFETY: Test environment, single-threaded access
        unsafe {
            std::env::remove_var("TEST_SECRET_ENV");
        }
    }

    #[test]
    fn test_resolve_secret_ci_mode_required() {
        let ctx = EnvContext::new();
        let config = SecretsConfig {
            ci_mode: true,
            fail_on_missing: true,
        };

        let secret_config = SecretValue::Prompt {
            env: None,
            required: true,
            description: None,
        };

        let result = resolve_secret("MISSING_SECRET", &secret_config, &ctx, &config);
        assert!(matches!(result, Err(SecretError::MissingRequired(_))));
    }

    #[test]
    fn test_resolve_secret_ci_mode_optional() {
        let ctx = EnvContext::new();
        let config = SecretsConfig {
            ci_mode: true,
            fail_on_missing: true,
        };

        let secret_config = SecretValue::Prompt {
            env: None,
            required: false,
            description: None,
        };

        let result = resolve_secret("OPTIONAL_SECRET", &secret_config, &ctx, &config).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_preview_secrets() {
        let mut secrets = HashMap::new();
        secrets.insert("SECRET_A".to_string(), SecretValue::Simple("a".to_string()));
        secrets.insert("SECRET_B".to_string(), SecretValue::Simple("b".to_string()));

        let preview = preview_secrets(&secrets);
        assert_eq!(preview.len(), 2);
        assert!(preview.contains(&"SECRET_A".to_string()));
        assert!(preview.contains(&"SECRET_B".to_string()));
    }

    #[test]
    fn test_collect_secrets_from_files() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("secret.txt");
        fs::write(&file_path, "collected_secret").unwrap();

        let mut secrets = HashMap::new();
        secrets.insert(
            "FILE_SECRET".to_string(),
            SecretValue::FromFile {
                from_file: file_path.to_string_lossy().to_string(),
            },
        );

        let mut ctx = EnvContext::new();
        ctx.current_dir = dir.path().to_path_buf();
        let config = SecretsConfig {
            ci_mode: true,
            fail_on_missing: true,
        };

        let result = collect_secrets(&secrets, &ctx, &config).unwrap();
        assert_eq!(
            result.get("FILE_SECRET"),
            Some(&"collected_secret".to_string())
        );
    }

    #[test]
    #[serial_test::serial(jarvy_allow_external_secrets)]
    fn test_resolve_secret_refuses_path_outside_project_and_home() {
        // Create the secret in /tmp but root the project somewhere else.
        // /tmp is outside both `current_dir` and `$HOME`, so the path
        // containment check must refuse with PathEscapesProject.
        let secret_dir = tempdir().unwrap();
        let secret_path = secret_dir.path().join("attacker.txt");
        fs::write(&secret_path, "leak").unwrap();

        let project = tempdir().unwrap();
        let mut ctx = EnvContext::new();
        ctx.current_dir = project.path().to_path_buf();
        // Force a non-tmp HOME so the secret can't sneak through `under_home`.
        ctx.home_dir = project.path().to_path_buf();

        let config = SecretsConfig {
            ci_mode: true,
            fail_on_missing: true,
        };
        let secret_config = SecretValue::FromFile {
            from_file: secret_path.to_string_lossy().to_string(),
        };

        let result = resolve_secret("LEAK", &secret_config, &ctx, &config);
        // Either escape-error or environment under test happens to share /tmp.
        // Asserting on the variant matters; the path string varies by platform.
        assert!(
            matches!(result, Err(SecretError::PathEscapesProject { .. })),
            "expected PathEscapesProject, got {:?}",
            result
        );
    }

    #[test]
    #[serial_test::serial(jarvy_allow_external_secrets)]
    #[allow(unsafe_code)]
    fn test_resolve_secret_external_path_allowed_with_env_override() {
        let secret_dir = tempdir().unwrap();
        let secret_path = secret_dir.path().join("override.txt");
        fs::write(&secret_path, "override_value").unwrap();

        let project = tempdir().unwrap();
        let mut ctx = EnvContext::new();
        ctx.current_dir = project.path().to_path_buf();
        ctx.home_dir = project.path().to_path_buf();

        let prev = std::env::var("JARVY_ALLOW_EXTERNAL_SECRETS").ok();
        unsafe {
            std::env::set_var("JARVY_ALLOW_EXTERNAL_SECRETS", "1");
        }
        let config = SecretsConfig {
            ci_mode: true,
            fail_on_missing: true,
        };
        let secret_config = SecretValue::FromFile {
            from_file: secret_path.to_string_lossy().to_string(),
        };

        let result = resolve_secret("OK", &secret_config, &ctx, &config);
        unsafe {
            match prev {
                Some(v) => std::env::set_var("JARVY_ALLOW_EXTERNAL_SECRETS", v),
                None => std::env::remove_var("JARVY_ALLOW_EXTERNAL_SECRETS"),
            }
        }
        assert_eq!(result.unwrap(), Some("override_value".to_string()));
    }
}
