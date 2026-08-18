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
/// `.git/hooks/<name>` with no framework process in the loop.
///
/// Three shapes teams can mix:
/// 1. **Inline bodies** — `hooks.pre-commit = "..."` — shell body
///    pasted into `jarvy.toml`.
/// 2. **File references** — `hooks.pre-commit = { file = "..." }` —
///    path relative to the project root, refused if it traverses
///    outside the repo.
/// 3. **Folder scan** — `dir = "scripts/hooks"` — every file whose
///    name matches a known git hook stage (`pre-commit`, `pre-push`,
///    …) gets installed. Files with unknown names are ignored.
///
/// Explicit `hooks` entries WIN over `dir` for the same stage, so a
/// team can point at a shared folder and override one specific hook
/// inline. Jarvy stamps a `# managed by jarvy` marker into every
/// installed file so subsequent runs can safely rewrite its own
/// output — install refuses to overwrite an existing hook that lacks
/// the marker (protects a hand-authored `.git/hooks/pre-commit`).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct NativeConfig {
    /// Optional folder (relative to project root) to scan for hook
    /// scripts. Every file inside whose bare name matches a known git
    /// hook stage gets installed. Unknown filenames are ignored so a
    /// `scripts/hooks/README.md` sitting alongside real hooks doesn't
    /// error. See [`NativeConfig::hooks`] for per-stage overrides.
    #[serde(default)]
    pub dir: Option<String>,

    /// Map of `<hook-stage>` → hook source (inline body OR file path).
    /// Wins over [`NativeConfig::dir`] when both target the same stage.
    ///
    /// ```toml
    /// [git_hooks.native]
    /// dir = "scripts/hooks"                        # scan folder
    /// hooks.commit-msg = "#!/bin/sh\n…"           # inline override
    /// hooks.pre-push = { file = "ci/pre-push.sh" } # file override
    /// ```
    #[serde(default)]
    pub hooks: std::collections::BTreeMap<String, HookSource>,
}

/// A single hook's body. Deserialized untagged — the same TOML key
/// accepts either a string (inline body, backward compatible with the
/// old shape) or a `{ file = "..." }` table (path relative to the
/// project root).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum HookSource {
    /// Inline shell body. Backward-compat shape.
    Inline(String),
    /// Path to a hook script, relative to the project root. Absolute
    /// paths and `..` traversal are refused at resolve time.
    File { file: String },
}

impl HookSource {
    /// Convenience for the tests — build an inline entry without
    /// naming the variant.
    #[cfg(test)]
    pub fn inline(s: impl Into<String>) -> Self {
        Self::Inline(s.into())
    }
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

    /// Which git-hook stages to wire up via `pre-commit install -t <type>`.
    /// Default `["pre-commit"]` — matches bare `pre-commit install`.
    /// Set to `["pre-commit", "pre-push"]` to also run hooks on push
    /// (typical for slow tests that shouldn't gate every commit but
    /// must gate what leaves the machine).
    ///
    /// Values are validated against the bounded set enumerated by
    /// [`SUPPORTED_HOOK_TYPES`] — unknown values would flow to
    /// `pre-commit install -t <arbitrary>` as CLI flags, so unknown
    /// entries are refused at config-load rather than shell time.
    #[serde(default = "default_hook_types")]
    pub hook_types: Vec<String>,
}

impl Default for PreCommitConfig {
    fn default() -> Self {
        Self {
            version: None,
            config: default_precommit_config(),
            install_hooks: true,
            hook_types: default_hook_types(),
        }
    }
}

fn default_precommit_config() -> String {
    ".pre-commit-config.yaml".to_string()
}

fn default_hook_types() -> Vec<String> {
    vec!["pre-commit".to_string()]
}

/// Bounded set of hook stages the pre-commit framework knows about.
/// Values sourced from <https://pre-commit.com/#confining-hooks-to-run-at-certain-stages>
/// — kept in sync with the `pre-commit install --hook-type` CLI accepts.
///
/// Refusing unknown values at config-load prevents an attacker-authored
/// jarvy.toml (or a typo) from smuggling `--foo` past `install -t` as
/// an arbitrary flag.
pub const SUPPORTED_HOOK_TYPES: &[&str] = &[
    "pre-commit",
    "pre-merge-commit",
    "pre-push",
    "prepare-commit-msg",
    "commit-msg",
    "post-commit",
    "post-checkout",
    "post-merge",
    "post-rewrite",
    "pre-rebase",
];

/// Validate that every entry in `hook_types` is a known pre-commit hook
/// stage. Returns the offending value on first miss so callers can
/// surface it in the error.
pub fn validate_hook_types(hook_types: &[String]) -> Result<(), String> {
    for value in hook_types {
        if !SUPPORTED_HOOK_TYPES.contains(&value.as_str()) {
            return Err(value.clone());
        }
    }
    Ok(())
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

    #[test]
    fn default_hook_types_is_pre_commit_only() {
        let cfg = PreCommitConfig::default();
        assert_eq!(cfg.hook_types, vec!["pre-commit".to_string()]);
    }

    #[test]
    fn parses_pre_push_hook_type() {
        let toml_str = r#"
[pre_commit]
hook_types = ["pre-commit", "pre-push"]
"#;
        let cfg: GitHooksConfig = toml::from_str(toml_str).unwrap();
        let pc = cfg.pre_commit.expect("pre_commit block parsed");
        assert_eq!(
            pc.hook_types,
            vec!["pre-commit".to_string(), "pre-push".to_string()]
        );
    }

    #[test]
    fn validate_accepts_known_types() {
        assert!(
            validate_hook_types(&["pre-commit".into(), "pre-push".into(), "commit-msg".into()])
                .is_ok()
        );
    }

    #[test]
    fn validate_refuses_unknown_type() {
        let bad = vec!["pre-commit".into(), "--evil-flag".into()];
        assert_eq!(validate_hook_types(&bad), Err("--evil-flag".to_string()));
    }

    #[test]
    fn validate_refuses_typo() {
        assert_eq!(
            validate_hook_types(&["pre-push-typo".into()]),
            Err("pre-push-typo".to_string())
        );
    }

    #[test]
    fn native_parses_inline_body_backward_compat() {
        let toml_str = r#"
[native.hooks]
pre-commit = "cargo fmt --check\n"
"#;
        let cfg: GitHooksConfig = toml::from_str(toml_str).unwrap();
        let n = cfg.native.expect("native block parsed");
        assert_eq!(
            n.hooks.get("pre-commit"),
            Some(&HookSource::Inline("cargo fmt --check\n".to_string()))
        );
    }

    #[test]
    fn native_parses_file_reference() {
        let toml_str = r#"
[native.hooks]
pre-push = { file = "scripts/hooks/pre-push.sh" }
"#;
        let cfg: GitHooksConfig = toml::from_str(toml_str).unwrap();
        let n = cfg.native.expect("native block parsed");
        assert_eq!(
            n.hooks.get("pre-push"),
            Some(&HookSource::File {
                file: "scripts/hooks/pre-push.sh".to_string()
            })
        );
    }

    #[test]
    fn native_parses_dir_scan() {
        let toml_str = r#"
[native]
dir = "scripts/hooks"
"#;
        let cfg: GitHooksConfig = toml::from_str(toml_str).unwrap();
        let n = cfg.native.expect("native block parsed");
        assert_eq!(n.dir.as_deref(), Some("scripts/hooks"));
        assert!(n.hooks.is_empty());
    }

    #[test]
    fn native_parses_dir_plus_overrides() {
        let toml_str = r##"
[native]
dir = "scripts/hooks"

[native.hooks]
commit-msg = "#!/bin/sh\ninline body"
pre-push = { file = "ci/pre-push.sh" }
"##;
        let cfg: GitHooksConfig = toml::from_str(toml_str).unwrap();
        let n = cfg.native.expect("native block parsed");
        assert_eq!(n.dir.as_deref(), Some("scripts/hooks"));
        assert!(matches!(
            n.hooks.get("commit-msg"),
            Some(HookSource::Inline(_))
        ));
        assert!(matches!(
            n.hooks.get("pre-push"),
            Some(HookSource::File { .. })
        ));
    }
}
