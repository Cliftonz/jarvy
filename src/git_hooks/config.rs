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

    /// Optional git repository to clone/update and scan for hook
    /// scripts. Same stage-name matching as `dir` — filenames like
    /// `pre-commit`, `pre-push`, etc. inside the repo (or [`subpath`]
    /// within it) get installed. Enables a team to publish one canonical
    /// hooks repo and consume it from every project without copying
    /// scripts around.
    ///
    /// Supported URL shapes:
    /// - `github:owner/repo` — shorthand → `https://github.com/owner/repo.git`
    /// - `https://host/path/repo.git` — plain HTTPS clone
    /// - `git+https://host/path/repo.git` — accepted for parity with
    ///   `library_sources`
    ///
    /// [`ref_`] is REQUIRED — unpinned URLs are refused at parse time
    /// so a publisher can't silently rev the hook body a consumer runs.
    ///
    /// Trust: remote-fetched configs need `[git_hooks] allow_remote = true`
    /// to consume ANY `repo` source (mirrors `dir` — the gate is on the
    /// enclosing block, not per-field).
    ///
    /// [`subpath`]: NativeConfig::subpath
    /// [`ref_`]: NativeConfig::git_ref
    #[serde(default)]
    pub repo: Option<String>,

    /// Git ref (tag/branch/commit SHA) to check out. Required when
    /// [`repo`] is set. SHAs and `v`-prefixed tags are treated as
    /// immutable; branch names emit a `git_hooks.mutable_ref` warning
    /// event so operators see the risk.
    ///
    /// [`repo`]: NativeConfig::repo
    #[serde(default, rename = "ref")]
    pub git_ref: Option<String>,

    /// Optional subdirectory inside the cloned repo to scan. When
    /// unset, scans the repo root. Refused if it starts with `/` or
    /// contains `..` segments (same rules as [`dir`]).
    ///
    /// [`dir`]: NativeConfig::dir
    #[serde(default)]
    pub subpath: Option<String>,

    /// Map of `<hook-stage>` → hook source (inline body OR file path).
    /// Wins over [`NativeConfig::dir`] AND [`NativeConfig::repo`] when
    /// they target the same stage.
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

/// Supported hook frameworks. Only `Native` has a shipping handler
/// today. Husky and Lefthook are stubbed for auto-detection so configs
/// that name them get a clear "not yet supported" error rather than a
/// silent fall-through. The pre-commit framework was removed in v0.8 —
/// use `Native` with a `[git_hooks.native]` dir/file/inline shape
/// instead.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum HookFramework {
    Husky,
    Lefthook,
    Native,
}

impl HookFramework {
    pub fn as_str(self) -> &'static str {
        match self {
            HookFramework::Husky => "husky",
            HookFramework::Lefthook => "lefthook",
            HookFramework::Native => "native",
        }
    }
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
    fn parses_framework_kebab_case() {
        let toml_str = r#"framework = "native""#;
        let cfg: GitHooksConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.framework, Some(HookFramework::Native));
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
