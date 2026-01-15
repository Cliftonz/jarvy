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
        Self {
            ci_mode: std::env::var("CI").is_ok()
                || std::env::var("JARVY_CI").is_ok()
                || std::env::var("JARVY_TEST_MODE").is_ok(),
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
            let content = fs::read_to_string(&path)?;
            Ok(Some(content.trim().to_string()))
        }
        SecretValue::Prompt {
            env,
            required,
            description,
        } => {
            // First check environment variable if specified
            if let Some(env_var) = env {
                if let Ok(value) = std::env::var(env_var) {
                    if !value.is_empty() {
                        return Ok(Some(value));
                    }
                }
            }

            // Check if the secret itself is in environment
            if let Ok(value) = std::env::var(name) {
                if !value.is_empty() {
                    return Ok(Some(value));
                }
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
            if let Ok(value) = std::env::var(name) {
                if !value.is_empty() {
                    return Ok(Some(value));
                }
            }

            // Check the marker value as an env var too
            if marker != name {
                if let Ok(value) = std::env::var(marker) {
                    if !value.is_empty() {
                        return Ok(Some(value));
                    }
                }
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
pub fn load_secret_from_file(path: &Path) -> Result<String, SecretError> {
    if !path.exists() {
        return Err(SecretError::FileNotFound(path.display().to_string()));
    }
    let content = fs::read_to_string(path)?;
    Ok(content.trim().to_string())
}

/// Preview secrets that would be collected (for dry-run)
/// Returns secret names without actual values
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

        let ctx = EnvContext::new();
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

        let ctx = EnvContext::new();
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
}
