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
pub mod generator;
pub mod rules;
pub mod scanner;
pub mod version;

pub use rules::{Detection, ToolCategory, default_rules};

use std::collections::HashSet;
use std::path::Path;

/// Final suggestion suitable for emitting into a `[provisioner]` entry.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolSuggestion {
    pub name: String,
    pub version: String,
    pub reason: String,
    pub category: ToolCategory,
}

/// Result of an `analyze` call. Required / recommended / already_configured
/// stay separate so the renderer can choose how to present them.
#[derive(Debug, Default, serde::Serialize)]
pub struct DiscoverReport {
    pub detections: Vec<Detection>,
    pub required: Vec<ToolSuggestion>,
    pub recommended: Vec<ToolSuggestion>,
    pub already_configured: Vec<String>,
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
    let detections = rules::run(project_dir, &default_rules());
    let mut report = DiscoverReport {
        detections: detections.clone(),
        ..Default::default()
    };

    let mut recommended_seen: HashSet<String> = HashSet::new();

    for d in &detections {
        if already_configured.contains(&d.tool) {
            report.already_configured.push(d.tool.clone());
        } else if known_tools.contains(&d.tool) {
            report.required.push(ToolSuggestion {
                name: d.tool.clone(),
                version: d.version.clone().unwrap_or_else(|| "latest".to_string()),
                reason: format!("detected from {}", d.source),
                category: d.category,
            });
        }

        for suggested in &d.suggests {
            if already_configured.contains(suggested) {
                continue;
            }
            if !known_tools.contains(suggested) {
                continue;
            }
            if !recommended_seen.insert(suggested.clone()) {
                continue;
            }
            report.recommended.push(ToolSuggestion {
                name: suggested.clone(),
                version: "latest".to_string(),
                reason: format!("commonly used with {}", d.tool),
                category: ToolCategory::Dev,
            });
        }
    }

    report
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
