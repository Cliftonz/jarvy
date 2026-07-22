//! Monorepo/workspace support
//!
//! Walks up from the current directory to find a root `jarvy.toml` with a
//! `[workspace]` section. Merges root config with member-specific config.

use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

/// Workspace configuration section in jarvy.toml
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct WorkspaceConfig {
    /// Paths to workspace member directories (relative to root).
    /// Supports simple `*` globs (e.g. `apps/*`, `packages/*`) — the
    /// glob expands to every immediate subdirectory of the parent.
    /// Exact paths are still supported and take precedence: an exact
    /// `apps/web` AND a glob `apps/*` both selecting `apps/web` is
    /// deduplicated.
    #[serde(default)]
    pub members: Vec<String>,
    /// Glob patterns to exclude from member resolution. Applied after
    /// glob expansion, so `apps/*` + `exclude = ["apps/legacy"]` yields
    /// every sibling of `legacy` minus `legacy` itself.
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Sections that members inherit from root config.
    ///
    /// **Use [`Self::effective_inherit`] when actually merging** — an
    /// empty / omitted `inherit` list is treated as `["provisioner"]`
    /// so the common monorepo case (members share the root toolset)
    /// works without explicit config. Both the production setup
    /// resolver (`config.rs`) and the `jarvy workspace` CLI surface
    /// route through `effective_inherit` so they cannot disagree on
    /// what a member inherits (review item P0 #4 — previously the
    /// CLI display widened to provisioner but production did not).
    #[serde(default)]
    pub inherit: Vec<String>,
}

impl WorkspaceConfig {
    /// Returns the inherit list to USE for resolution. Empty in-source
    /// means "no explicit list — fall back to provisioner so the
    /// common case just works." Set `inherit = ["provisioner",
    /// "hooks"]` explicitly to opt in to additional sections, or
    /// `inherit = ["custom"]` to opt OUT of provisioner inheritance.
    pub fn effective_inherit(&self) -> Vec<String> {
        if self.inherit.is_empty() {
            vec!["provisioner".to_string()]
        } else {
            self.inherit.clone()
        }
    }

    /// Expand every `members` entry against the workspace root,
    /// turning `apps/*` glob entries into the list of immediate
    /// subdirectories matching the pattern. Then drop anything that
    /// matches a pattern in `self.exclude`.
    ///
    /// Returns relative paths (matching how members appear in the
    /// config), sorted + deduplicated. Path containment is enforced
    /// upstream by `commands::workspace_cmd::resolve_member` so glob
    /// expansion is allowed to produce literal `apps/legacy` style
    /// entries safely.
    pub fn resolved_members(&self, root: &Path) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for raw in &self.members {
            if raw.contains('*') {
                if let Some(parent) = parent_of_glob(raw) {
                    let parent_path = root.join(&parent);
                    let suffix = raw.strip_prefix(&parent).unwrap_or(raw);
                    let suffix = suffix.trim_start_matches('/');
                    if suffix != "*" {
                        // Only the simple `parent/*` shape is supported;
                        // anything more complex (parent/*/x) is left
                        // verbatim — the user can still use exact paths.
                        out.push(raw.clone());
                        continue;
                    }
                    if let Ok(entries) = std::fs::read_dir(&parent_path) {
                        for entry in entries.flatten() {
                            if !entry.path().is_dir() {
                                continue;
                            }
                            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                                continue;
                            };
                            // Skip hidden / dotfile dirs — `apps/.git`,
                            // `apps/.idea` aren't intended members.
                            if name.starts_with('.') {
                                continue;
                            }
                            if parent.is_empty() {
                                out.push(name);
                            } else {
                                out.push(format!("{parent}/{name}"));
                            }
                        }
                    }
                }
            } else {
                out.push(raw.clone());
            }
        }
        out.retain(|m| !self.is_excluded(m));
        out.sort();
        out.dedup();
        out
    }

    fn is_excluded(&self, member: &str) -> bool {
        self.exclude.iter().any(|pat| glob_matches(pat, member))
    }
}

/// Return the literal-path prefix of a glob, i.e. everything before
/// the first `*`. `"apps/*"` → `"apps"`, `"packages/*"` → `"packages"`,
/// `"*"` → `""`.
fn parent_of_glob(pattern: &str) -> Option<String> {
    let star = pattern.find('*')?;
    let prefix = &pattern[..star];
    Some(prefix.trim_end_matches('/').to_string())
}

/// Minimal `*`-only glob matcher. Supports `*` (any path-component
/// run) but not `**`, `?`, or character classes. Sufficient for the
/// `apps/*` / `packages/*-server` style patterns workspace users
/// actually write.
fn glob_matches(pattern: &str, candidate: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == candidate;
    }
    let mut pos = 0;
    let first = parts[0];
    if !candidate.starts_with(first) {
        return false;
    }
    pos += first.len();
    for (i, part) in parts[1..].iter().enumerate() {
        if part.is_empty() {
            // Trailing or consecutive `*` — accept any remainder.
            if i + 1 == parts.len() - 1 {
                return true;
            }
            continue;
        }
        match candidate[pos..].find(part) {
            Some(idx) => pos += idx + part.len(),
            None => return false,
        }
    }
    // Last part must match the tail of the candidate (anchored).
    let last = parts.last().copied().unwrap_or("");
    if last.is_empty() {
        return true;
    }
    candidate.ends_with(last)
}

/// Resolved workspace context
#[derive(Debug, Clone)]
pub struct WorkspaceContext {
    /// Path to the workspace root jarvy.toml
    pub root_config: PathBuf,
    /// Path to the member jarvy.toml (if in a member directory)
    #[allow(dead_code)] // Exposed for callers; not used internally yet.
    pub member_config: Option<PathBuf>,
    /// The workspace configuration
    pub workspace: WorkspaceConfig,
    /// Which member we're currently in (if any)
    pub current_member: Option<String>,
}

/// Walk up from `start` to find a jarvy.toml with a [workspace] section.
/// Returns None if no workspace root is found.
pub fn find_workspace_root(start: &Path) -> Option<WorkspaceContext> {
    let mut current = start.to_path_buf();

    loop {
        let config_path = current.join("jarvy.toml");
        if config_path.exists()
            && let Ok(content) = std::fs::read_to_string(&config_path)
            && let Ok(parsed) = toml::from_str::<toml::Value>(&content)
            && let Some(ws) = parsed.get("workspace")
            && let Ok(workspace) = toml::Value::try_into::<WorkspaceConfig>(ws.clone())
        {
            // Found a workspace root. Determine if `start` is a member.
            let current_member = determine_member(start, &current, &workspace);
            let member_config = current_member
                .as_ref()
                .map(|m| current.join(m).join("jarvy.toml"))
                .filter(|p| p.exists());

            return Some(WorkspaceContext {
                root_config: config_path,
                member_config,
                workspace,
                current_member,
            });
        }

        if !current.pop() {
            break;
        }
    }

    None
}

/// Determine which workspace member the given path is in (if any).
///
/// Membership is decided by path-component prefix, not string prefix:
/// a member named `app` does NOT match a directory called `apple`.
fn determine_member(target: &Path, root: &Path, workspace: &WorkspaceConfig) -> Option<String> {
    let relative = target.strip_prefix(root).ok()?;

    let target_components: Vec<Component<'_>> = relative.components().collect();

    for member in &workspace.members {
        let member_components: Vec<Component<'_>> = Path::new(member).components().collect();
        if member_components.is_empty() {
            continue;
        }
        if target_components.len() < member_components.len() {
            continue;
        }
        if target_components[..member_components.len()] == member_components[..] {
            return Some(member.clone());
        }
    }

    None
}

/// Merge a root config TOML value with a member config TOML value.
/// The `inherit` list controls which top-level sections are inherited.
/// Member values override root values on conflict.
pub fn merge_configs(root: &toml::Value, member: &toml::Value, inherit: &[String]) -> toml::Value {
    let mut merged = member.clone();

    let Some(root_table) = root.as_table() else {
        return merged;
    };
    let Some(merged_table) = merged.as_table_mut() else {
        return merged;
    };

    for section in inherit {
        if !merged_table.contains_key(section) {
            if let Some(root_val) = root_table.get(section) {
                merged_table.insert(section.clone(), root_val.clone());
            }
        } else if section == "provisioner" {
            // For provisioner, merge tool-by-tool (member overrides root)
            if let (Some(root_tools), Some(merged_tools)) = (
                root_table.get(section).and_then(|v| v.as_table()),
                merged_table.get_mut(section).and_then(|v| v.as_table_mut()),
            ) {
                for (tool, version) in root_tools {
                    if !merged_tools.contains_key(tool) {
                        merged_tools.insert(tool.clone(), version.clone());
                    }
                }
            }
        }
    }

    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_configs_inherits_missing_sections() {
        let root: toml::Value = toml::from_str(
            r#"
            [provisioner]
            git = "latest"
            node = "20"

            [env.vars]
            FOO = "bar"
            "#,
        )
        .unwrap();

        let member: toml::Value = toml::from_str(
            r#"
            [provisioner]
            python = "3.12"
            "#,
        )
        .unwrap();

        let merged = merge_configs(&root, &member, &["provisioner".into(), "env".into()]);
        let table = merged.as_table().unwrap();

        // Member's python should be there
        let prov = table.get("provisioner").unwrap().as_table().unwrap();
        assert!(prov.contains_key("python"));
        // Root's git/node should be inherited
        assert!(prov.contains_key("git"));
        assert!(prov.contains_key("node"));
        // Root's env should be inherited
        assert!(table.contains_key("env"));
    }

    /// PRD-047 phase 2 — `apps/*` expands to each immediate subdirectory.
    #[test]
    fn resolved_members_expands_glob() {
        let tmp = tempfile::tempdir().unwrap();
        for sub in ["apps/web", "apps/api", "apps/.git"] {
            std::fs::create_dir_all(tmp.path().join(sub)).unwrap();
        }
        let ws = WorkspaceConfig {
            members: vec!["apps/*".to_string()],
            exclude: vec![],
            inherit: vec![],
        };
        let resolved = ws.resolved_members(tmp.path());
        assert_eq!(
            resolved,
            vec!["apps/api".to_string(), "apps/web".to_string()]
        );
    }

    #[test]
    fn resolved_members_applies_exclude_patterns() {
        let tmp = tempfile::tempdir().unwrap();
        for sub in ["apps/web", "apps/api", "apps/legacy"] {
            std::fs::create_dir_all(tmp.path().join(sub)).unwrap();
        }
        let ws = WorkspaceConfig {
            members: vec!["apps/*".to_string()],
            exclude: vec!["apps/legacy".to_string()],
            inherit: vec![],
        };
        let resolved = ws.resolved_members(tmp.path());
        assert!(!resolved.contains(&"apps/legacy".to_string()));
        assert!(resolved.contains(&"apps/web".to_string()));
        assert!(resolved.contains(&"apps/api".to_string()));
    }

    #[test]
    fn resolved_members_dedups_exact_and_glob_overlap() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("apps/web")).unwrap();
        let ws = WorkspaceConfig {
            members: vec!["apps/web".to_string(), "apps/*".to_string()],
            exclude: vec![],
            inherit: vec![],
        };
        let resolved = ws.resolved_members(tmp.path());
        assert_eq!(resolved, vec!["apps/web".to_string()]);
    }

    #[test]
    fn glob_matches_table() {
        assert!(glob_matches("apps/*", "apps/web"));
        assert!(glob_matches("*-server", "api-server"));
        assert!(!glob_matches("*-server", "api-client"));
        assert!(glob_matches("apps/web", "apps/web"));
        assert!(!glob_matches("apps/web", "apps/api"));
    }

    #[test]
    fn determine_member_does_not_match_prefix_collision() {
        // Reproduces the previous string-prefix bug: members=["app"] should NOT
        // match a path like "apple/main.rs". Path-component matching prevents this.
        let workspace = WorkspaceConfig {
            members: vec!["app".to_string()],
            exclude: vec![],
            inherit: vec![],
        };
        let root = Path::new("/repo");
        let target = Path::new("/repo/apple/main.rs");
        assert_eq!(determine_member(target, root, &workspace), None);
    }

    #[test]
    fn determine_member_matches_exact_first_component() {
        let workspace = WorkspaceConfig {
            members: vec!["app".to_string(), "service-a".to_string()],
            exclude: vec![],
            inherit: vec![],
        };
        let root = Path::new("/repo");
        assert_eq!(
            determine_member(Path::new("/repo/app"), root, &workspace),
            Some("app".to_string())
        );
        assert_eq!(
            determine_member(Path::new("/repo/app/src/main.rs"), root, &workspace),
            Some("app".to_string())
        );
        assert_eq!(
            determine_member(Path::new("/repo/service-a/Cargo.toml"), root, &workspace),
            Some("service-a".to_string())
        );
    }

    #[test]
    fn determine_member_handles_multi_segment_member_path() {
        let workspace = WorkspaceConfig {
            members: vec!["packages/web".to_string()],
            exclude: vec![],
            inherit: vec![],
        };
        let root = Path::new("/repo");
        assert_eq!(
            determine_member(Path::new("/repo/packages/web/index.html"), root, &workspace),
            Some("packages/web".to_string())
        );
        // `packages/webex` should NOT match member `packages/web`.
        assert_eq!(
            determine_member(Path::new("/repo/packages/webex/x"), root, &workspace),
            None
        );
    }

    #[test]
    fn find_workspace_root_returns_none_outside_workspace() {
        let tmp = tempfile::TempDir::new().unwrap();
        // No jarvy.toml at all.
        assert!(find_workspace_root(tmp.path()).is_none());
    }

    #[test]
    fn find_workspace_root_finds_root_from_member_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let app_dir = root.join("app");
        std::fs::create_dir_all(&app_dir).unwrap();
        std::fs::write(
            root.join("jarvy.toml"),
            r#"
[workspace]
members = ["app"]
inherit = ["provisioner"]

[provisioner]
git = "latest"
"#,
        )
        .unwrap();
        let ctx = find_workspace_root(&app_dir).expect("workspace should be found");
        assert_eq!(ctx.current_member.as_deref(), Some("app"));
        assert_eq!(ctx.workspace.members, vec!["app".to_string()]);
        assert_eq!(ctx.root_config, root.join("jarvy.toml"));
    }

    #[test]
    fn merge_configs_inherits_explicit_section() {
        let root: toml::Value = toml::from_str(
            r#"
            [drift]
            enabled = true
            version_policy = "minor"
            "#,
        )
        .unwrap();
        let member: toml::Value = toml::from_str(
            r#"
            [provisioner]
            git = "latest"
            "#,
        )
        .unwrap();
        let merged = merge_configs(&root, &member, &["drift".into()]);
        let table = merged.as_table().unwrap();
        assert!(table.contains_key("drift"));
        let drift = table.get("drift").unwrap().as_table().unwrap();
        assert_eq!(drift.get("enabled").unwrap().as_bool(), Some(true));
    }

    #[test]
    fn test_member_overrides_root() {
        let root: toml::Value = toml::from_str(
            r#"
            [provisioner]
            node = "18"
            "#,
        )
        .unwrap();

        let member: toml::Value = toml::from_str(
            r#"
            [provisioner]
            node = "20"
            "#,
        )
        .unwrap();

        let merged = merge_configs(&root, &member, &["provisioner".into()]);
        let prov = merged
            .as_table()
            .unwrap()
            .get("provisioner")
            .unwrap()
            .as_table()
            .unwrap();
        assert_eq!(prov.get("node").unwrap().as_str().unwrap(), "20");
    }
}
