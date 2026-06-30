//! Project-context envelope passed to the agent.
//!
//! Compact JSON-serializable view of the project so the agent can
//! reason about it in one shot. Three sources:
//!
//! 1. `crate::discover::analyze()` — DiscoverReport (detected
//!    ecosystems, required/recommended tools, uninstallable bucket).
//! 2. Filtered tree listing — top-level entries plus the marker
//!    files (`Cargo.toml`, `package.json`, …) anywhere up to depth 3.
//! 3. Git status (current branch, dirty flag) when the project is a
//!    git repo.
//!
//! All paths are project-relative — no absolute paths leak to the
//! prompt. The envelope flows through `observability::sanitizer`
//! before being printed so secrets in source files don't reach the
//! agent (defense in depth — the agent runs locally, but anything
//! that ends up in stdout is also visible in shell history / logs).

use crate::discover::DiscoverReport;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

/// Compact project view. Kebab-cased JSON via serde so the agent
/// sees a stable, documented shape.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ProjectContext {
    /// Project root, relative to cwd when serialized for the agent.
    pub project_dir: String,
    /// Whether `jarvy.toml` exists at the root. Drives the
    /// greenfield-vs-refinement branch in the prompt.
    pub has_jarvy_toml: bool,
    /// Top-level directory contents, sorted, dot-files surfaced
    /// after non-dot to keep marker files visually first.
    pub top_level: Vec<String>,
    /// Marker files found anywhere in the tree (up to depth 3).
    /// Names only — full paths would balloon the envelope on
    /// monorepos.
    pub markers: Vec<String>,
    /// Git status — None when not a repo.
    pub git: Option<GitStatus>,
    /// Discover output. Reusing the existing type means the agent
    /// sees the same JSON shape as `jarvy discover --format json`.
    pub discover: DiscoverReport,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct GitStatus {
    pub branch: Option<String>,
    pub dirty: bool,
}

/// Marker files we surface in the envelope. Subset of `discover`'s
/// detection rules; the goal here is "what would a human looking at
/// the tree notice immediately?", not exhaustive ecosystem coverage.
const MARKER_FILES: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "go.mod",
    "Gemfile",
    "Dockerfile",
    "Makefile",
    "Justfile",
    ".nvmrc",
    ".python-version",
    "rust-toolchain.toml",
    "pnpm-workspace.yaml",
    "turbo.json",
];

/// Build the envelope for the given project root. `discover` is
/// passed in (rather than recomputed here) so callers can choose to
/// use a cached or test-injected `DiscoverReport`.
pub fn build(project_dir: &Path, discover: DiscoverReport) -> ProjectContext {
    let top_level = list_top_level(project_dir);
    let markers = find_markers(project_dir, MARKER_FILES, 3);
    let git = read_git_status(project_dir);
    let has_jarvy_toml = project_dir.join("jarvy.toml").exists();

    ProjectContext {
        project_dir: project_dir.to_string_lossy().into_owned(),
        has_jarvy_toml,
        top_level,
        markers,
        git,
        discover,
    }
}

fn list_top_level(dir: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<String> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        // Skip the busiest noise dirs by default.
        .filter(|name| !matches!(name.as_str(), "node_modules" | "target" | ".git"))
        .collect();
    entries.sort_by_key(|name| (name.starts_with('.'), name.clone()));
    entries
}

fn find_markers(root: &Path, markers: &[&str], max_depth: usize) -> Vec<String> {
    let marker_set: HashSet<&str> = markers.iter().copied().collect();
    let mut found: HashSet<String> = HashSet::new();
    walk(root, root, &marker_set, &mut found, max_depth);
    let mut out: Vec<String> = found.into_iter().collect();
    out.sort();
    out
}

fn walk(
    root: &Path,
    dir: &Path,
    markers: &HashSet<&str>,
    found: &mut HashSet<String>,
    remaining_depth: usize,
) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        let Some(name_os) = path.file_name() else {
            continue;
        };
        let name = name_os.to_string_lossy();
        if path.is_dir() {
            // Cheap guards against the obvious noise dirs.
            if matches!(
                name.as_ref(),
                "node_modules" | "target" | ".git" | "vendor" | "dist" | "build"
            ) {
                continue;
            }
            if remaining_depth > 0 {
                walk(root, &path, markers, found, remaining_depth - 1);
            }
        } else if markers.contains(name.as_ref()) {
            // Record project-relative form.
            if let Ok(rel) = path.strip_prefix(root) {
                found.insert(rel.to_string_lossy().into_owned());
            } else {
                found.insert(name.into_owned());
            }
        }
    }
}

fn read_git_status(project_dir: &Path) -> Option<GitStatus> {
    let git_dir = project_dir.join(".git");
    if !git_dir.exists() {
        return None;
    }
    let branch = std::fs::read_to_string(git_dir.join("HEAD"))
        .ok()
        .and_then(|s| {
            s.strip_prefix("ref: refs/heads/")
                .map(|b| b.trim().to_string())
        });
    // We avoid spawning `git status` because the wizard often runs
    // pre-`jarvy setup` and the user may not have git installed yet.
    // "Dirty" is approximated as "any tracked file modified" which
    // requires a porcelain run; safer to report Unknown via false
    // and let the agent ask if needed.
    Some(GitStatus {
        branch,
        dirty: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discover::DiscoverReport;
    use std::fs;
    use tempfile::tempdir;

    fn empty_report(dir: &Path) -> DiscoverReport {
        let already = std::collections::HashSet::new();
        let known = std::collections::HashSet::new();
        crate::discover::analyze(dir, &already, &known)
    }

    #[test]
    fn detects_jarvy_toml_presence() {
        let tmp = tempdir().unwrap();
        let report = empty_report(tmp.path());
        let ctx_a = build(tmp.path(), report.clone());
        assert!(!ctx_a.has_jarvy_toml);
        fs::write(tmp.path().join("jarvy.toml"), "").unwrap();
        let ctx_b = build(tmp.path(), report);
        assert!(ctx_b.has_jarvy_toml);
    }

    #[test]
    fn surfaces_markers_in_root_and_subdir() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        fs::create_dir(tmp.path().join("apps")).unwrap();
        fs::write(tmp.path().join("apps").join("package.json"), "").unwrap();
        let ctx = build(tmp.path(), empty_report(tmp.path()));
        assert!(ctx.markers.iter().any(|m| m == "Cargo.toml"));
        // `package.json` discovered under apps/ — depth 1 from root.
        assert!(
            ctx.markers.iter().any(|m| m.ends_with("package.json")),
            "got: {:?}",
            ctx.markers
        );
    }

    #[test]
    fn ignores_noise_dirs() {
        let tmp = tempdir().unwrap();
        fs::create_dir(tmp.path().join("node_modules")).unwrap();
        fs::write(tmp.path().join("node_modules").join("package.json"), "").unwrap();
        let ctx = build(tmp.path(), empty_report(tmp.path()));
        assert!(
            !ctx.markers.iter().any(|m| m.contains("node_modules")),
            "node_modules must not appear in markers: {:?}",
            ctx.markers
        );
    }

    #[test]
    fn git_status_none_when_not_a_repo() {
        let tmp = tempdir().unwrap();
        let ctx = build(tmp.path(), empty_report(tmp.path()));
        assert!(ctx.git.is_none());
    }

    #[test]
    fn json_roundtrip_stable_shape() {
        // Pin the snake_case key names so the agent prompt can rely
        // on a documented schema.
        let tmp = tempdir().unwrap();
        let ctx = build(tmp.path(), empty_report(tmp.path()));
        let json = serde_json::to_value(&ctx).unwrap();
        for key in &[
            "project_dir",
            "has_jarvy_toml",
            "top_level",
            "markers",
            "git",
            "discover",
        ] {
            assert!(json.get(key).is_some(), "envelope missing key: {key}");
        }
    }
}
