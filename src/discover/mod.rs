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
#[derive(Debug, Default, serde::Serialize)]
pub struct DiscoverReport {
    pub detections: Vec<Detection>,
    pub required: Vec<ToolSuggestion>,
    pub recommended: Vec<ToolSuggestion>,
    pub already_configured: Vec<String>,
    /// Detected tools jarvy doesn't yet have an installer for. Empty
    /// when every detection mapped cleanly to a known tool.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uninstallable: Vec<UninstallableSuggestion>,
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
        if already_configured.contains(&d.tool) {
            // Pinned at any version: if it already satisfies the
            // detected version (or detection has no version), this
            // is a clean already-configured. Otherwise, surface as
            // override-suggestion.
            let detected_version = d.version.as_deref();
            let pinned = already_configured_versions.get(&d.tool).map(|s| s.as_str());
            if version_already_satisfies(pinned, detected_version) {
                already_seen.push(d.tool.clone());
            } else if known_tools.contains(&d.tool) {
                required.push(ToolSuggestion {
                    name: d.tool.clone(),
                    version: detected_version.unwrap_or("latest").to_string(),
                    reason: format!(
                        "detected from {} (pinned `{}` is more lax)",
                        d.source,
                        pinned.unwrap_or("?")
                    ),
                    category: d.category,
                });
            }
        } else if known_tools.contains(&d.tool) {
            required.push(ToolSuggestion {
                name: d.tool.clone(),
                version: d.version.clone().unwrap_or_else(|| "latest".to_string()),
                reason: format!("detected from {}", d.source),
                category: d.category,
            });
        } else {
            uninstallable.push(UninstallableSuggestion {
                name: d.tool.clone(),
                source: d.source.clone(),
                category: d.category,
                reason: "no jarvy handler".to_string(),
            });
        }

        for suggested in &d.suggests {
            if already_configured.contains(suggested) {
                continue;
            }
            if !known_tools.contains(suggested) {
                continue;
            }
            if !recommended_seen.insert(suggested.as_str()) {
                continue;
            }
            recommended.push(ToolSuggestion {
                name: suggested.clone(),
                version: "latest".to_string(),
                reason: format!("commonly used with {}", d.tool),
                category: ToolCategory::Dev,
            });
        }
    }

    DiscoverReport {
        detections,
        required,
        recommended,
        already_configured: already_seen,
        uninstallable,
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
