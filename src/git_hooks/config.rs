//! `[git_hooks]` configuration types.

use serde::{Deserialize, Serialize};

/// `[git_hooks]` block.
///
/// `Default` is hand-implemented to match the serde-deserialized
/// defaults (`enabled = true`, `auto_install = true`). Previously the
/// auto-derived `Default` produced `enabled = false`, which made
/// `Option<GitHooksConfig>::unwrap_or_default()` (used by
/// `commands/hooks_cmd.rs`) silently disable hooks for projects
/// without a `[git_hooks]` block — even when a `.pre-commit-config.yaml`
/// existed. Review item 14 (P1).
#[derive(Debug, Clone, Deserialize, Serialize)]
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

    /// Native git hook scripts written straight into `.git/hooks/`
    /// — no framework process between git and your script. Keyed by
    /// hook stage (`pre-commit`, `commit-msg`, …).
    #[serde(default)]
    pub native: Option<NativeConfig>,

    /// Origin tag set by the config loader; not serialized. Propagated
    /// by `Config::mark_remote` so handlers can enforce the
    /// `allow_remote` gate without re-reading the parent `Config`.
    /// Review item 5 (P0) — previously the field was missing entirely,
    /// making `allow_remote` dead code.
    #[serde(skip)]
    pub origin: crate::ai_hooks::ConfigOrigin,
}

impl Default for GitHooksConfig {
    /// Matches the serde-deserialized defaults. Previously the
    /// auto-derived `Default` produced `enabled = false`, breaking
    /// `unwrap_or_default()` call sites. Review item 14 (P1).
    fn default() -> Self {
        Self {
            enabled: true,
            framework: None,
            auto_install: true,
            auto_update: false,
            run_after_install: false,
            allow_remote: false,
            pre_commit: None,
            native: None,
            origin: crate::ai_hooks::ConfigOrigin::Local,
        }
    }
}

impl crate::ai_hooks::HasOrigin for GitHooksConfig {
    fn set_origin(&mut self, origin: crate::ai_hooks::ConfigOrigin) {
        self.origin = origin;
    }
}

/// `[git_hooks.native]` block — write hook scripts directly into
/// `.git/hooks/<name>` with no framework process in the loop. Each
/// entry is a hook stage name → inline shell body. Jarvy stamps a
/// `# managed by jarvy` marker into the file so a future run can
/// recognize / overwrite its own output without clobbering hooks the
/// user wrote by hand.
///
/// The most useful failure mode: if the existing `.git/hooks/<name>`
/// has DIFFERENT content and lacks the Jarvy marker, install refuses
/// with `HookError::InstallFailed` so we never silently overwrite
/// hand-rolled hooks.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NativeConfig {
    /// Map of `<hook-stage>` → inline shell body. Example:
    ///
    /// ```toml
    /// [git_hooks.native]
    /// hooks.pre-commit = """
    /// #!/bin/sh
    /// cargo fmt --check || exit 1
    /// """
    /// hooks.commit-msg = "..."
    /// ```
    #[serde(default)]
    pub hooks: std::collections::BTreeMap<String, String>,
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
        let parsed: GitHooksConfig = toml::from_str("").unwrap();
        // Both paths must agree (review item 14 — the previously-
        // documented quirk was a footgun).
        assert!(parsed.enabled);
        assert!(parsed.auto_install);
        assert!(!parsed.auto_update);
        assert!(!parsed.run_after_install);
        assert!(!parsed.allow_remote);
        assert!(c.enabled, "Default::default must match serde defaults");
        assert!(c.auto_install);
        assert!(!c.auto_update);
        assert!(!c.run_after_install);
        assert!(!c.allow_remote);
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

    /// Origin defaults to Local — propagation from Config::mark_remote
    /// is tested in src/config.rs::tests.
    #[test]
    fn origin_defaults_to_local() {
        let cfg: GitHooksConfig = toml::from_str("").unwrap();
        assert_eq!(cfg.origin, crate::ai_hooks::ConfigOrigin::Local);
    }
}
