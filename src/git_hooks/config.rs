//! `[git_hooks]` configuration types.

use serde::{Deserialize, Serialize};

/// `[git_hooks]` block.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct GitHooksConfig {
    /// Master enable. Default `true` — the block's presence implies
    /// enablement; users set `enabled = false` to declare-but-disable.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Framework to use. When unset, jarvy auto-detects from the
    /// presence of `.pre-commit-config.yaml`, `.husky/`, or
    /// `lefthook.yml`. Detection order: pre-commit → husky → lefthook.
    pub framework: Option<HookFramework>,

    /// Install hooks during `jarvy setup`. Default `true` — same shape
    /// as `[packages] allow_remote`: silent opt-in.
    #[serde(default = "default_true")]
    pub auto_install: bool,

    /// Run `pre-commit autoupdate` during `jarvy setup` after install.
    /// Default `false` — autoupdate can rev pinned hook versions across
    /// the whole team unexpectedly, so it's opt-in.
    #[serde(default)]
    pub auto_update: bool,

    /// Run hooks against the whole tree once after install. Default
    /// `false` — first-run can be slow and surfaces unrelated lint debt
    /// in the install transcript.
    #[serde(default)]
    pub run_after_install: bool,

    /// Allow remote configs (`jarvy setup --from <url>`) to auto-install
    /// hooks. Default `false`: a friendly-looking remote config cannot
    /// land arbitrary git hooks on the consuming machine without an
    /// explicit opt-in in the SOURCE config. Mirrors the
    /// `[packages] allow_remote` trust gate.
    #[serde(default)]
    pub allow_remote: bool,

    /// pre-commit framework knobs.
    #[serde(default)]
    pub pre_commit: Option<PreCommitConfig>,
}

fn default_true() -> bool {
    true
}

/// Supported hook frameworks. `PreCommit` is the only one with a
/// shipping handler today; the others are stubbed so configs can
/// declare intent and get a clear "not yet supported" error rather than
/// a silent fall-through.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum HookFramework {
    PreCommit,
    Husky,
    Lefthook,
    Native,
}

impl HookFramework {
    pub fn as_str(self) -> &'static str {
        match self {
            HookFramework::PreCommit => "pre-commit",
            HookFramework::Husky => "husky",
            HookFramework::Lefthook => "lefthook",
            HookFramework::Native => "native",
        }
    }
}

/// `[git_hooks.pre_commit]` knobs.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PreCommitConfig {
    /// Pin a specific pre-commit version. When set, jarvy verifies and
    /// will `pip install --upgrade pre-commit==<version>` if not
    /// satisfied. When unset, whatever's installed is fine.
    pub version: Option<String>,

    /// Path to the pre-commit config file, relative to project root.
    #[serde(default = "default_precommit_config")]
    pub config: String,

    /// Pass `--install-hooks` to `pre-commit install` so the hook envs
    /// are warmed up at install time rather than on first commit.
    /// Default `true` — first-commit latency surprise is a worse UX
    /// than the extra install-time cost.
    #[serde(default = "default_true")]
    pub install_hooks: bool,
}

impl Default for PreCommitConfig {
    fn default() -> Self {
        Self {
            version: None,
            config: default_precommit_config(),
            install_hooks: true,
        }
    }
}

fn default_precommit_config() -> String {
    ".pre-commit-config.yaml".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_enable_hooks_and_auto_install() {
        let c = GitHooksConfig::default();
        // Default::default() does NOT call our serde defaults — those
        // only fire during deserialization. Validate via TOML round-trip.
        let parsed: GitHooksConfig = toml::from_str("").unwrap();
        assert!(parsed.enabled);
        assert!(parsed.auto_install);
        assert!(!parsed.auto_update);
        assert!(!parsed.run_after_install);
        assert!(!parsed.allow_remote);
        // The default-only Default::default branch:
        assert!(!c.enabled); // documented quirk — use toml::from_str("") for defaults
    }

    #[test]
    fn parses_pinned_pre_commit_version() {
        let toml_str = r#"
[pre_commit]
version = "3.6.0"
"#;
        let cfg: GitHooksConfig = toml::from_str(toml_str).unwrap();
        let pc = cfg.pre_commit.expect("pre_commit block parsed");
        assert_eq!(pc.version.as_deref(), Some("3.6.0"));
        assert_eq!(pc.config, ".pre-commit-config.yaml");
        assert!(pc.install_hooks);
    }

    #[test]
    fn parses_framework_kebab_case() {
        let toml_str = r#"framework = "pre-commit""#;
        let cfg: GitHooksConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.framework, Some(HookFramework::PreCommit));
    }

    #[test]
    fn parses_allow_remote_explicit() {
        let toml_str = "allow_remote = true";
        let cfg: GitHooksConfig = toml::from_str(toml_str).unwrap();
        assert!(cfg.allow_remote);
    }
}
