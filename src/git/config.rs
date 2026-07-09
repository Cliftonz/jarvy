//! Git configuration types for parsing `[git]` section of jarvy.toml

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Git configuration section in jarvy.toml
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct GitConfig {
    /// User name (plain string or from environment)
    #[serde(default)]
    pub user_name: Option<ConfigValue>,

    /// User email (plain string or from environment)
    #[serde(default)]
    pub user_email: Option<ConfigValue>,

    /// Enable commit signing
    #[serde(default)]
    pub signing: bool,

    /// Path to signing key (SSH public key or GPG key ID)
    #[serde(default)]
    pub signing_key: Option<String>,

    /// Signing format (auto-detected if not specified)
    #[serde(default)]
    pub signing_format: Option<SigningFormat>,

    /// Default branch name for git init
    #[serde(default)]
    pub default_branch: Option<String>,

    /// Enable pull.rebase
    #[serde(default)]
    pub pull_rebase: bool,

    /// Enable rebase.autoStash
    #[serde(default)]
    pub auto_stash: bool,

    /// Enable push.autoSetupRemote
    #[serde(default)]
    pub push_autosetup: bool,

    /// Editor for git commit messages
    #[serde(default)]
    pub editor: Option<String>,

    /// Line ending handling (core.autocrlf)
    #[serde(default)]
    pub autocrlf: Option<AutoCrlf>,

    /// Line ending style (core.eol)
    #[serde(default)]
    pub eol: Option<String>,

    /// Credential helper (auto-detected if not specified)
    #[serde(default)]
    pub credential_helper: Option<String>,

    /// Configuration scope (global or local)
    #[serde(default)]
    pub scope: ConfigScope,

    /// Apply Jarvy's default git config for keys the user left unset. Host-aware:
    /// `core.autocrlf` (`true` on Windows, `input` elsewhere), `core.longpaths`
    /// (`true` on Windows), `core.precomposeunicode` (`true` on macOS). Plus a
    /// cross-platform recommended set: `fetch.prune`, `rerere.enabled`,
    /// `merge.conflictStyle = zdiff3`. `None`/absent = enabled (mirrors the
    /// always-on per-OS `credential.helper` default); set `os_defaults = false`
    /// to opt out. Explicit typed fields (e.g. `autocrlf`) and `[git.extra]`
    /// entries always win over these.
    #[serde(default)]
    pub os_defaults: Option<bool>,

    /// Git aliases
    #[serde(default)]
    pub aliases: HashMap<String, String>,

    /// Free-form escape hatch for git config keys Jarvy doesn't model as
    /// first-class fields (e.g. `core.fsmonitor`, `feature.manyFiles`,
    /// `diff.colorMoved`). Keys are dotted git config keys (`section.key` or
    /// `section.subsection.key`); values are written verbatim via
    /// `git config <scope> <key> <value>`. Applied AFTER the typed keys, so an
    /// entry here overrides a modeled field if both target the same key.
    ///
    /// Values still pass through the `!`-shell refusal in `set_config`, and
    /// keys are validated (`validate_extra_key`) to reject flag-injection and
    /// malformed grammar. `.gitconfig` semantics — no first-party analogue in
    /// jarvy.toml — are the only reason this exists; prefer a typed field.
    #[serde(default)]
    pub extra: HashMap<String, String>,
}

/// Configuration value - can be plain string or sourced from environment
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ConfigValue {
    /// Plain string value
    Plain(String),
    /// Value sourced from environment variable with optional default
    FromEnv {
        /// Environment variable name
        env: String,
        /// Default value if env var not set
        #[serde(default)]
        default: Option<String>,
    },
}

impl ConfigValue {
    /// Resolve the config value, reading from environment if needed
    pub fn resolve(&self) -> Option<String> {
        match self {
            ConfigValue::Plain(s) => Some(s.clone()),
            ConfigValue::FromEnv { env, default } => {
                std::env::var(env).ok().or_else(|| default.clone())
            }
        }
    }
}

/// Configuration scope - global (~/.gitconfig) or local (.git/config)
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConfigScope {
    /// Global git configuration (~/.gitconfig)
    #[default]
    Global,
    /// Local repository configuration (.git/config)
    Local,
}

/// Signing format for commits
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SigningFormat {
    /// SSH key signing
    Ssh,
    /// GPG key signing
    Gpg,
}

/// Line ending autocrlf settings
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AutoCrlf {
    /// Convert LF to CRLF on checkout, CRLF to LF on commit (Windows)
    True,
    /// No conversion
    False,
    /// Convert CRLF to LF on commit only (Unix/macOS)
    Input,
}

impl AutoCrlf {
    /// Convert to git config value string
    pub fn as_str(&self) -> &'static str {
        match self {
            AutoCrlf::True => "true",
            AutoCrlf::False => "false",
            AutoCrlf::Input => "input",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_value_plain() {
        let value = ConfigValue::Plain("John Doe".to_string());
        assert_eq!(value.resolve(), Some("John Doe".to_string()));
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_config_value_from_env() {
        // SAFETY: test-only, single-threaded access to env var
        unsafe { std::env::set_var("TEST_GIT_USER", "Jane Doe") };

        let value = ConfigValue::FromEnv {
            env: "TEST_GIT_USER".to_string(),
            default: None,
        };
        assert_eq!(value.resolve(), Some("Jane Doe".to_string()));

        unsafe { std::env::remove_var("TEST_GIT_USER") };
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_config_value_from_env_with_default() {
        // SAFETY: test-only, single-threaded access to env var
        unsafe { std::env::remove_var("TEST_GIT_USER_MISSING") };

        let value = ConfigValue::FromEnv {
            env: "TEST_GIT_USER_MISSING".to_string(),
            default: Some("Default User".to_string()),
        };
        assert_eq!(value.resolve(), Some("Default User".to_string()));
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_config_value_from_env_no_default() {
        // SAFETY: test-only, single-threaded access to env var
        unsafe { std::env::remove_var("TEST_GIT_USER_NONE") };

        let value = ConfigValue::FromEnv {
            env: "TEST_GIT_USER_NONE".to_string(),
            default: None,
        };
        assert_eq!(value.resolve(), None);
    }

    #[test]
    fn test_autocrlf_as_str() {
        assert_eq!(AutoCrlf::True.as_str(), "true");
        assert_eq!(AutoCrlf::False.as_str(), "false");
        assert_eq!(AutoCrlf::Input.as_str(), "input");
    }

    #[test]
    fn test_config_scope_default() {
        let scope = ConfigScope::default();
        assert_eq!(scope, ConfigScope::Global);
    }

    #[test]
    fn test_git_config_parsing() {
        let toml_str = r#"
user_name = "John Doe"
user_email = { env = "GIT_EMAIL", default = "john@example.com" }
signing = true
signing_key = "~/.ssh/id_ed25519.pub"
default_branch = "main"
pull_rebase = true
auto_stash = true
push_autosetup = true
editor = "vim"
autocrlf = "input"
scope = "global"
os_defaults = false

[aliases]
co = "checkout"
br = "branch"
ci = "commit"
st = "status"

[extra]
"core.fsmonitor" = "true"
"feature.manyFiles" = "true"
"#;
        let config: GitConfig = toml::from_str(toml_str).expect("Failed to parse config");

        assert!(matches!(config.user_name, Some(ConfigValue::Plain(_))));
        assert!(matches!(
            config.user_email,
            Some(ConfigValue::FromEnv { .. })
        ));
        assert!(config.signing);
        assert_eq!(
            config.signing_key,
            Some("~/.ssh/id_ed25519.pub".to_string())
        );
        assert_eq!(config.default_branch, Some("main".to_string()));
        assert!(config.pull_rebase);
        assert!(config.auto_stash);
        assert!(config.push_autosetup);
        assert_eq!(config.editor, Some("vim".to_string()));
        assert_eq!(config.autocrlf, Some(AutoCrlf::Input));
        assert_eq!(config.scope, ConfigScope::Global);
        assert_eq!(config.os_defaults, Some(false));
        assert_eq!(config.aliases.len(), 4);
        assert_eq!(config.aliases.get("co"), Some(&"checkout".to_string()));
        assert_eq!(config.extra.len(), 2);
        assert_eq!(
            config.extra.get("core.fsmonitor"),
            Some(&"true".to_string())
        );
    }
}
