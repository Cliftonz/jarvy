//! Project tool auto-discovery (PRD-044 MVP).
//!
//! Scans a project directory for marker files (`Cargo.toml`, `package.json`,
//! `Dockerfile`, `k8s/`, ...) and emits a suggested `jarvy.toml` so new
//! contributors get a complete environment without guessing what's needed.
//!
//! The detection engine is built-in for v1 — custom rule files
//! (`.jarvy/discovery-rules.yaml`) are intentionally deferred. Adding a new
//! detection rule today is one entry in [`default_rules`] in `rules.rs`.
//!
//! # Public API
//!
//! ```ignore
//! let report = discover::analyze(project_dir);
//! for s in &report.required {
//!     println!("{} = {}", s.name, s.version);
//! }
//! ```
//!
//! `jarvy discover` (CLI) is a thin wrapper over [`analyze`].

pub mod commands;
pub mod config;
pub mod generator;
pub mod rules;
pub mod scanner;
pub mod version;

pub use rules::{Detection, ToolCategory, default_rules};

use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Final suggestion suitable for emitting into a `[provisioner]` entry.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolSuggestion {
    pub name: String,
    pub version: String,
    pub reason: String,
    pub category: ToolCategory,
}

/// A tool jarvy can't actually install but detection still found.
/// Surfaces so users know what's missing without us pretending we can
/// fill the gap. Future ecosystems (Java, .NET, Gradle, …) land here
/// until first-party handlers ship.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UninstallableSuggestion {
    pub name: String,
    pub source: String,
    pub category: ToolCategory,
    /// Why jarvy can't install this — typically
    /// `"no jarvy handler"` or `"requires custom rule"`.
    pub reason: String,
}

/// Result of an `analyze` call. Required / recommended / already_configured
/// stay separate so the renderer can choose how to present them.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct DiscoverReport {
    pub detections: Vec<Detection>,
    pub required: Vec<ToolSuggestion>,
    pub recommended: Vec<ToolSuggestion>,
    pub already_configured: Vec<String>,
    /// Detected tools jarvy doesn't yet have an installer for. Empty
    /// when every detection mapped cleanly to a known tool.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uninstallable: Vec<UninstallableSuggestion>,
    /// How many `recommended` companion entries were suppressed
    /// because they were also present in `required` (their own
    /// direct marker fired). Emitted on the `discover.applied`
    /// telemetry event so operators can distinguish "rule bug never
    /// generated the recommendation" from "recommendation dropped in
    /// favour of the stronger required signal" without running with
    /// debug logging. Skipped from JSON output when zero to keep the
    /// common case tidy.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub recommended_dropped_dup: usize,
}

fn is_zero_usize(n: &usize) -> bool {
    *n == 0
}

/// Scan `project_dir` and resolve detections to suggestions. Pure function
/// — no I/O beyond reading marker files; safe to call from any thread.
///
/// `already_configured` is supplied by the caller (typically the parsed
/// `[provisioner]` keys of an existing jarvy.toml) so the renderer can
/// surface what's new vs. what's already pinned.
///
/// `known_tools` is the canonical jarvy tool registry (lowercase names).
/// Suggestions whose name isn't in `known_tools` are dropped — we never
/// recommend installing a tool that jarvy can't actually install.
pub fn analyze(
    project_dir: &Path,
    already_configured: &HashSet<String>,
    known_tools: &HashSet<String>,
) -> DiscoverReport {
    analyze_with(
        project_dir,
        already_configured,
        &HashMap::new(),
        known_tools,
        default_rules(),
    )
}

/// Full-fat `analyze` variant. Beyond the public-API `analyze`:
///
/// - `already_configured_versions` lets the caller pass each pinned
///   tool's version string so we can suppress suggestions that ALREADY
///   satisfy the detected requirement (review: per-language version
///   range narrowing — e.g. existing `node = "^20.0.0"` already
///   satisfies a `.nvmrc` of `20`, so don't re-suggest `node = "20"`).
/// - `rules` is the rule set to apply. Defaults to `default_rules()`
///   but a custom rule file (`[discover] rules = "..."`) can extend
///   the engine without touching jarvy itself.
pub fn analyze_with(
    project_dir: &Path,
    already_configured: &HashSet<String>,
    already_configured_versions: &HashMap<String, String>,
    known_tools: &HashSet<String>,
    rules: &[rules::DetectionRule],
) -> DiscoverReport {
    let detections = rules::run(project_dir, rules);

    let mut required: Vec<ToolSuggestion> = Vec::new();
    let mut recommended: Vec<ToolSuggestion> = Vec::new();
    let mut already_seen: Vec<String> = Vec::new();
    let mut uninstallable: Vec<UninstallableSuggestion> = Vec::new();
    let mut recommended_seen: HashSet<&str> = HashSet::new();

    for d in &detections {
        let d_tool: &str = d.tool.as_ref();
        if already_configured.contains(d_tool) {
            // Pinned at any version: if it already satisfies the
            // detected version (or detection has no version), this
            // is a clean already-configured. Otherwise, surface as
            // override-suggestion.
            let detected_version = d.version.as_deref();
            let pinned = already_configured_versions.get(d_tool).map(|s| s.as_str());
            if version_already_satisfies(pinned, detected_version) {
                already_seen.push(d_tool.to_string());
            } else if known_tools.contains(d_tool) {
                required.push(ToolSuggestion {
                    name: d_tool.to_string(),
                    version: detected_version.unwrap_or("latest").to_string(),
                    reason: format!(
                        "detected from {} (pinned `{}` is more lax)",
                        d.source,
                        pinned.unwrap_or("?")
                    ),
                    category: d.category,
                });
            }
        } else if known_tools.contains(d_tool) {
            required.push(ToolSuggestion {
                name: d_tool.to_string(),
                version: d.version.clone().unwrap_or_else(|| "latest".to_string()),
                reason: format!("detected from {}", d.source),
                category: d.category,
            });
        } else {
            uninstallable.push(UninstallableSuggestion {
                name: d_tool.to_string(),
                source: d.source.as_ref().to_string(),
                category: d.category,
                reason: "no jarvy handler".to_string(),
            });
        }

        for suggested in &d.suggests {
            let suggested_str: &str = suggested.as_ref();
            if already_configured.contains(suggested_str) {
                continue;
            }
            if !known_tools.contains(suggested_str) {
                continue;
            }
            if !recommended_seen.insert(suggested_str) {
                continue;
            }
            recommended.push(ToolSuggestion {
                name: suggested_str.to_string(),
                version: "latest".to_string(),
                reason: format!("commonly used with {}", d.tool),
                category: ToolCategory::Dev,
            });
        }
    }

    // A tool that qualifies as both required (via its own marker) AND
    // recommended (as a companion of some other detection) would land
    // twice in the generated `[provisioner]` block — which produces a
    // duplicate-key TOML parse error on the next `jarvy discover` /
    // `jarvy validate`. Required wins, so filter it out of
    // recommended. Concrete case: `release-plz.toml` present (required)
    // AND `.github/` present (release-plz is a companion of gh).
    //
    // Linear scan over `required` rather than a HashSet: with `required`
    // typically < 20 entries, `iter().any()` beats a HashSet build
    // (RandomState init + bucket allocation) on wall time AND drops the
    // allocation from the hot path (Perf F7).
    //
    // Emit a debug-level `discover.recommended_dropped` event for each
    // suppressed recommendation so on-call answering "why isn't
    // release-plz listed as a companion?" can distinguish "dropped in
    // favour of a required entry" from "rule bug never generated it"
    // (Obs F6). Also surface a `recommended_dropped_dup` count on the
    // parent `discover.applied` event via the returned report's
    // `recommended_dropped_dup` field.
    let mut recommended_dropped_dup: usize = 0;
    recommended.retain(|s| {
        let is_dup = required.iter().any(|r| r.name == s.name);
        if is_dup {
            recommended_dropped_dup += 1;
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::debug!(
                    event = "discover.recommended_dropped",
                    name = %s.name,
                    promoted_to = "required",
                );
            }
        }
        !is_dup
    });

    DiscoverReport {
        detections,
        required,
        recommended,
        already_configured: already_seen,
        uninstallable,
        recommended_dropped_dup,
    }
}

/// Per-language version range narrowing.
///
/// Returns `true` when `pinned` already covers the `detected` version.
/// Conservative: when version info is missing on EITHER side we say
/// `true` so the user's hand-curated pin survives the discover pass.
/// We only flip to `false` (i.e. "your pin is too lax — here's a
/// suggested update") when we have both a pinned spec AND a detected
/// version AND the matcher can decide that the detected version
/// doesn't satisfy the pin.
fn version_already_satisfies(pinned: Option<&str>, detected: Option<&str>) -> bool {
    let Some(pinned) = pinned else {
        // Caller said "tool is configured" but didn't tell us the
        // version. Trust the caller — no override.
        return true;
    };
    let Some(detected) = detected else {
        // No detected version pin — anything goes.
        return true;
    };
    let trimmed = pinned.trim();
    if trimmed.is_empty() || trimmed == "latest" || trimmed == "*" || trimmed == detected {
        return true;
    }
    // Cheap structural check: if the pinned spec starts with a range
    // operator (`^`, `~`, `>=`, `<`, `=`, `>`), assume it's a semver
    // expression and ask the project's own semver matcher.
    crate::tools::version::version_satisfies(detected, trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn registry_with(names: &[&str]) -> HashSet<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn detects_rust_from_cargo_toml() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "[package]\nname=\"x\"").unwrap();
        let report = analyze(
            tmp.path(),
            &HashSet::new(),
            &registry_with(&["rust", "cargo-watch"]),
        );
        let names: Vec<&str> = report.required.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"rust"), "got {names:?}");
    }

    #[test]
    fn detects_node_from_package_json() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("package.json"), "{}").unwrap();
        let report = analyze(tmp.path(), &HashSet::new(), &registry_with(&["node"]));
        let names: Vec<&str> = report.required.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"node"));
    }

    #[test]
    fn drops_unknown_tools() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        // `rust` not in the registry → dropped.
        let report = analyze(tmp.path(), &HashSet::new(), &HashSet::new());
        assert!(report.required.is_empty());
    }

    #[test]
    fn marks_already_configured_separately() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        let mut configured = HashSet::new();
        configured.insert("rust".to_string());
        let report = analyze(tmp.path(), &configured, &registry_with(&["rust"]));
        assert!(report.required.is_empty());
        assert_eq!(report.already_configured, vec!["rust"]);
    }

    #[test]
    fn version_inferred_from_marker_file() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        fs::write(
            tmp.path().join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.85.0\"",
        )
        .unwrap();
        let report = analyze(tmp.path(), &HashSet::new(), &registry_with(&["rust"]));
        let rust = report.required.iter().find(|s| s.name == "rust").unwrap();
        assert_eq!(rust.version, "1.85.0");
    }

    /// Version-narrowing — when the user has already pinned a semver
    /// range that COVERS the detected version, don't re-suggest.
    #[test]
    fn pinned_range_covering_detected_version_does_not_re_suggest() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        fs::write(
            tmp.path().join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.85.0\"",
        )
        .unwrap();
        let mut configured: HashSet<String> = HashSet::new();
        configured.insert("rust".to_string());
        let mut versions: HashMap<String, String> = HashMap::new();
        versions.insert("rust".to_string(), ">= 1.80, < 2.0".to_string());

        let report = analyze_with(
            tmp.path(),
            &configured,
            &versions,
            &registry_with(&["rust"]),
            default_rules(),
        );
        assert_eq!(report.already_configured, vec!["rust"]);
        assert!(report.required.is_empty(), "got {:?}", report.required);
    }

    /// Version-narrowing — when the pinned spec does NOT cover the
    /// detected version, surface as an override-suggestion.
    #[test]
    fn pinned_range_below_detected_version_surfaces_as_override() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        fs::write(
            tmp.path().join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.85.0\"",
        )
        .unwrap();
        let mut configured: HashSet<String> = HashSet::new();
        configured.insert("rust".to_string());
        let mut versions: HashMap<String, String> = HashMap::new();
        versions.insert("rust".to_string(), "1.70.0".to_string());

        let report = analyze_with(
            tmp.path(),
            &configured,
            &versions,
            &registry_with(&["rust"]),
            default_rules(),
        );
        let names: Vec<&str> = report.required.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"rust"), "got required={names:?}");
    }

    /// Uninstallable bucket — detected tools we don't have a handler
    /// for surface separately, not silently dropped.
    #[test]
    fn unknown_tool_lands_in_uninstallable_bucket() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        // Empty registry: every detection is uninstallable.
        let report = analyze(tmp.path(), &HashSet::new(), &HashSet::new());
        assert!(report.required.is_empty());
        let names: Vec<&str> = report
            .uninstallable
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert!(names.contains(&"rust"), "got {names:?}");
    }

    // `detects_git_from_dot_git_dir` was folded into the
    // `dir_marker_rules_detect_from_canonical_dirs` data-driven table.
    // See that test for the git / gh / vscode dir markers.

    /// A `.github/` directory (workflows, issue templates, CODEOWNERS)
    /// should surface `gh` as a required tool AND recommend
    /// `release-plz` as a companion (release-plz is
    /// GitHub-Action-driven).
    #[test]
    fn detects_gh_from_dot_github_dir() {
        let tmp = tempdir().unwrap();
        fs::create_dir(tmp.path().join(".github")).unwrap();
        let report = analyze(
            tmp.path(),
            &HashSet::new(),
            &registry_with(&["gh", "release-plz"]),
        );
        let required: Vec<&str> = report.required.iter().map(|s| s.name.as_str()).collect();
        assert!(required.contains(&"gh"), "got required={required:?}");
        let recommended: Vec<&str> = report
            .recommended
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert!(
            recommended.contains(&"release-plz"),
            "got recommended={recommended:?}"
        );
    }

    // `detects_release_plz_from_marker_toml` folded into the
    // niche-languages file-marker table. The `.github/` +
    // release-plz.toml overlap is separately covered by
    // `required_drops_dup_from_recommended`.

    /// Niche language coverage: every language with a first-party
    /// jarvy tool must have a detection rule that resolves against
    /// its canonical marker file. Data-driven so adding a new
    /// language only requires one entry — no per-language `#[test]`.
    #[test]
    fn niche_languages_detect_from_canonical_markers() {
        // (tool_name, marker_file_at_root)
        let cases: &[(&str, &str)] = &[
            ("deno", "deno.json"),
            ("elixir", "mix.exs"),
            ("erlang", "rebar.config"),
            ("haskell", "cabal.project"),
            ("crystal", "shard.yml"),
            ("gleam", "gleam.toml"),
            ("lua", ".lua-version"),
            ("luarocks", "example.rockspec"),
            ("nim", "example.nimble"),
            ("ocaml", "dune-project"),
            ("scala", "build.sbt"),
            ("zig", "build.zig"),
            ("julia", "Manifest.toml"),
            ("cmake", "CMakeLists.txt"),
            ("skaffold", "skaffold.yaml"),
            ("bazelisk", "MODULE.bazel"),
            // Additional file-marker rules previously covered only via
            // e2e matrix rows — now pinned at the fast unit-test tier
            // so removing e.g. `bun.lockb` from the bun rule trips
            // immediately, not after the slow e2e run.
            ("bun", "bun.lockb"),
            ("infisical", ".infisical.json"),
            ("release-plz", "release-plz.toml"),
            ("composer", "composer.json"),
        ];
        let known = registry_with(&[
            "deno", "elixir", "erlang", "haskell", "crystal", "gleam", "lua", "luarocks",
            "nim", "ocaml", "scala", "zig", "julia", "cmake", "skaffold", "bazelisk", "bun",
            "infisical", "release-plz", "composer", "php",
        ]);
        for (tool, marker) in cases {
            let tmp = tempdir().unwrap();
            fs::write(tmp.path().join(marker), "").unwrap();
            let report = analyze(tmp.path(), &HashSet::new(), &known);
            let names: Vec<&str> = report.required.iter().map(|s| s.name.as_str()).collect();
            assert!(
                names.contains(tool),
                "{tool} not required after writing {marker}; got required={names:?}"
            );
        }
    }

    /// Dir-marker detections — `.git`, `.github`, `.vscode` — kept in
    /// their own table because `fs::create_dir` isn't `fs::write`.
    /// Data-driven for the same reason as the file-marker table:
    /// adding a new dir marker (e.g. `.svn`, `.hg`) is one row.
    #[test]
    fn dir_marker_rules_detect_from_canonical_dirs() {
        let cases: &[(&str, &str)] = &[
            ("git", ".git"),
            ("gh", ".github"),
            ("vscode", ".vscode"),
        ];
        let known = registry_with(&["git", "gh", "vscode", "release-plz"]);
        for (tool, dir) in cases {
            let tmp = tempdir().unwrap();
            fs::create_dir(tmp.path().join(dir)).unwrap();
            let report = analyze(tmp.path(), &HashSet::new(), &known);
            let names: Vec<&str> = report.required.iter().map(|s| s.name.as_str()).collect();
            assert!(
                names.contains(tool),
                "{tool} not required after creating {dir}/; got required={names:?}"
            );
        }
    }

    /// Regression guard: hostile repos could try to smuggle detection
    /// triggers by planting marker files INSIDE `.git/` — a
    /// `.git/package.json`, `.git/mix.exs`, etc. The current scanner
    /// only checks the top-level directory (see
    /// `find_first_match` — no recursion), so this is defense-in-depth
    /// against a future refactor that walks into subdirs. If the scan
    /// starts recursing without excluding `.git/`, this test fires.
    #[test]
    fn discover_does_not_walk_into_dot_git() {
        let tmp = tempdir().unwrap();
        fs::create_dir(tmp.path().join(".git")).unwrap();
        // Plant hostile "markers" inside .git/ that would trigger
        // node / elixir rules if the scanner walked into them.
        fs::write(tmp.path().join(".git/package.json"), "{}").unwrap();
        fs::write(tmp.path().join(".git/mix.exs"), "").unwrap();
        fs::write(tmp.path().join(".git/Cargo.toml"), "[package]\nname=\"x\"").unwrap();

        let report = analyze(
            tmp.path(),
            &HashSet::new(),
            &registry_with(&["git", "node", "elixir", "rust"]),
        );
        let names: Vec<&str> = report.required.iter().map(|s| s.name.as_str()).collect();
        // git rule fires on `.git/` — that's expected.
        assert!(names.contains(&"git"), "got {names:?}");
        // But NONE of the languages implied by files INSIDE .git/
        // should surface — the scanner must not walk into it.
        for hostile in ["node", "elixir", "rust"] {
            assert!(
                !names.contains(&hostile),
                "hostile marker inside `.git/{hostile}-marker` must NOT \
                 trigger `{hostile}` detection — scanner must stay at \
                 project root. Got: {names:?}"
            );
        }
    }

    /// Elixir → Erlang companion: BEAM-targeting languages need the
    /// Erlang runtime installed even when their own marker is
    /// present. Guards against dropping `suggests: ["erlang"]` from
    /// the elixir rule during a future edit.
    #[test]
    fn elixir_recommends_erlang_companion() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("mix.exs"), "").unwrap();
        let report = analyze(
            tmp.path(),
            &HashSet::new(),
            &registry_with(&["elixir", "erlang"]),
        );
        let recommended: Vec<&str> = report
            .recommended
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert!(recommended.contains(&"erlang"), "got {recommended:?}");
    }

    /// Julia disambiguation: `Manifest.toml` is Julia-specific, but
    /// `Project.toml` is NOT — many tools use that filename. Pin the
    /// rule so we don't regress into detecting every `Project.toml`
    /// as a Julia project.
    #[test]
    fn julia_does_not_falsely_match_bare_project_toml() {
        let tmp = tempdir().unwrap();
        // `Project.toml` alone (no `Manifest.toml`, no
        // `JuliaProject.toml`) must NOT trigger julia detection.
        fs::write(tmp.path().join("Project.toml"), "").unwrap();
        let report = analyze(tmp.path(), &HashSet::new(), &registry_with(&["julia"]));
        let names: Vec<&str> = report.required.iter().map(|s| s.name.as_str()).collect();
        assert!(
            !names.contains(&"julia"),
            "bare `Project.toml` must NOT imply Julia (too ambiguous); \
             got required={names:?}"
        );
    }

    /// Polyglot end-to-end: a repo with Cargo.toml + package.json +
    /// composer.json + go.mod plus lockfiles for pnpm and yarn (the
    /// Node side of a monorepo sometimes ships both to accomodate
    /// tooling in different workspaces) should surface every language
    /// runtime AND its lockfile-implied package manager as required,
    /// with rust companions (bacon, cargo-nextest) and go companions
    /// (golangci-lint, air, delve) as recommendations.
    ///
    /// This is the exact scenario the audit asked about — pins the
    /// full detection contract for a Node+PHP+Rust+Go project.
    #[test]
    fn polyglot_node_php_rust_go_detects_full_stack() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "[package]\nname=\"x\"").unwrap();
        fs::write(tmp.path().join("package.json"), "{}").unwrap();
        fs::write(tmp.path().join("pnpm-lock.yaml"), "").unwrap();
        fs::write(tmp.path().join("composer.json"), "{}").unwrap();
        fs::write(tmp.path().join("go.mod"), "module x\ngo 1.22\n").unwrap();

        let known = registry_with(&[
            "rust",
            "node",
            "php",
            "go",
            "pnpm",
            "composer",
            "bacon",
            "cargo-nextest",
            "golangci-lint",
            "air",
            "delve",
        ]);
        let report = analyze(tmp.path(), &HashSet::new(), &known);

        let required: Vec<&str> = report.required.iter().map(|s| s.name.as_str()).collect();
        for expected in ["rust", "node", "php", "go", "pnpm", "composer"] {
            assert!(
                required.contains(&expected),
                "polyglot required must include {expected}; got {required:?}"
            );
        }

        let recommended: Vec<&str> = report
            .recommended
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        for expected in ["bacon", "cargo-nextest", "golangci-lint", "air", "delve"] {
            assert!(
                recommended.contains(&expected),
                "polyglot recommended must include {expected}; got {recommended:?}"
            );
        }
    }

    /// Lockfile precision: only `pnpm-lock.yaml` present → require
    /// `pnpm`, not `yarn`. Prevents regressing the split from
    /// "node.suggests = [pnpm, yarn]" (both always) to per-lockfile
    /// required tools.
    #[test]
    fn only_pnpm_lockfile_requires_pnpm_not_yarn() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("package.json"), "{}").unwrap();
        fs::write(tmp.path().join("pnpm-lock.yaml"), "").unwrap();
        let report = analyze(
            tmp.path(),
            &HashSet::new(),
            &registry_with(&["node", "pnpm", "yarn"]),
        );
        let required: Vec<&str> = report.required.iter().map(|s| s.name.as_str()).collect();
        assert!(required.contains(&"pnpm"), "got {required:?}");
        assert!(
            !required.contains(&"yarn"),
            "yarn.lock absent — should not be required; got {required:?}"
        );
    }

    /// PHP without a framework: only composer.json triggers php +
    /// composer detection, no false positives.
    #[test]
    fn composer_json_alone_requires_php_and_composer() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("composer.json"), "{}").unwrap();
        let report = analyze(
            tmp.path(),
            &HashSet::new(),
            &registry_with(&["php", "composer"]),
        );
        let required: Vec<&str> = report.required.iter().map(|s| s.name.as_str()).collect();
        assert!(required.contains(&"php"), "got {required:?}");
        assert!(required.contains(&"composer"), "got {required:?}");
    }

    /// When a tool qualifies as BOTH required (own marker) AND
    /// recommended (companion of another detection), required wins
    /// and the recommendation is dropped — otherwise the generated
    /// `[provisioner]` block writes the same key twice, producing a
    /// duplicate-key TOML parse error on the next discover / validate
    /// pass.
    ///
    /// Data-driven: each row is one overlap scenario. Adding a new
    /// rule whose `suggests` overlaps with another rule's own-marker
    /// is one row here plus a paired rules.rs entry — no per-scenario
    /// `#[test]` boilerplate.
    #[test]
    fn required_drops_dup_from_recommended() {
        struct DedupCase {
            /// Files to `touch` in the fixture root.
            markers: &'static [&'static str],
            /// Dirs to `create_dir` in the fixture root.
            dirs: &'static [&'static str],
            /// Tool that MUST land in `required` AND MUST NOT land in
            /// `recommended` (would double-write in `[provisioner]`).
            tool: &'static str,
            /// Human-readable why-does-this-overlap explanation for
            /// failure output.
            why: &'static str,
        }

        let cases: &[DedupCase] = &[
            DedupCase {
                markers: &["release-plz.toml"],
                dirs: &[".github"],
                tool: "release-plz",
                why: "own marker `release-plz.toml` + companion via `.github` \
                      (gh suggests release-plz)",
            },
            // Node lockfile precision: if a future refactor makes the
            // node rule suggest pnpm/yarn AGAIN, the presence of the
            // matching lockfile would trigger dedup — pin the invariant.
            DedupCase {
                markers: &["package.json", "pnpm-lock.yaml"],
                dirs: &[],
                tool: "pnpm",
                why: "own marker `pnpm-lock.yaml` — must dedup vs. \
                      any future node-side suggest of pnpm",
            },
            DedupCase {
                markers: &["package.json", "yarn.lock"],
                dirs: &[],
                tool: "yarn",
                why: "own marker `yarn.lock` — same rationale for yarn",
            },
        ];

        for case in cases {
            let tmp = tempdir().unwrap();
            for dir in case.dirs {
                fs::create_dir(tmp.path().join(dir)).unwrap();
            }
            for marker in case.markers {
                fs::write(tmp.path().join(marker), "").unwrap();
            }
            let report = analyze(
                tmp.path(),
                &HashSet::new(),
                &registry_with(&[
                    "gh",
                    "release-plz",
                    "node",
                    "pnpm",
                    "yarn",
                    "bun",
                ]),
            );
            let required: Vec<&str> =
                report.required.iter().map(|s| s.name.as_str()).collect();
            let recommended: Vec<&str> =
                report.recommended.iter().map(|s| s.name.as_str()).collect();
            assert!(
                required.contains(&case.tool),
                "{}: expected `{}` in required (own marker present); \
                 got required={:?} recommended={:?}",
                case.why,
                case.tool,
                required,
                recommended
            );
            assert!(
                !recommended.contains(&case.tool),
                "{}: `{}` must be dedup'd out of recommended (would \
                 duplicate the `[provisioner]` key); got recommended={:?}",
                case.why,
                case.tool,
                recommended
            );
        }
    }

    #[test]
    fn recommends_companion_tools_once() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("Dockerfile"), "FROM alpine").unwrap();
        fs::write(tmp.path().join("docker-compose.yml"), "version: '3'").unwrap();
        let report = analyze(
            tmp.path(),
            &HashSet::new(),
            &registry_with(&["docker", "docker-compose", "lazydocker"]),
        );
        // docker-compose suggested by both Dockerfile and compose; only once.
        let count = report
            .recommended
            .iter()
            .filter(|s| s.name == "docker-compose")
            .count();
        assert_eq!(count, 1, "got {:?}", report.recommended);
    }
}
