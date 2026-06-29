//! Monorepo/workspace support
//!
//! Walks up from the current directory to find a root `jarvy.toml` with a
//! `[workspace]` section. Merges root config with member-specific config.

use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

/// Workspace configuration section in jarvy.toml
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct WorkspaceConfig {
    /// Paths to workspace member directories (relative to root)
    #[serde(default)]
    pub members: Vec<String>,
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
        if config_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Ok(parsed) = toml::from_str::<toml::Value>(&content) {
                    if let Some(ws) = parsed.get("workspace") {
                        if let Ok(workspace) = toml::Value::try_into::<WorkspaceConfig>(ws.clone())
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
                    }
                }
            }
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

    #[test]
    fn determine_member_does_not_match_prefix_collision() {
        // Reproduces the previous string-prefix bug: members=["app"] should NOT
        // match a path like "apple/main.rs". Path-component matching prevents this.
        let workspace = WorkspaceConfig {
            members: vec!["app".to_string()],
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
