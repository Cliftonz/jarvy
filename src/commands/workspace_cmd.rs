//! `jarvy workspace` CLI handler (PRD-047 monorepo support).
//!
//! Surfaces three read-only subcommands over the existing `crate::workspace`
//! foundation (`find_workspace_root`, `merge_configs`):
//!
//! - `list`     — enumerate members with their resolved tool sets
//! - `show`     — pretty-print one member's resolved config + inheritance
//! - `validate` — sanity check each member (config parses, dir exists)
//!
//! Read-only by design. Workspace-aware `jarvy setup --project <name>`
//! orchestration is intentionally deferred — surfacing the workspace
//! structure first lets users debug inheritance before we add a
//! command that actually mutates the environment based on it.

use crate::cli::WorkspaceAction;
use crate::workspace;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub fn run_workspace(action: &WorkspaceAction, file: &str) -> i32 {
    let project_dir = Path::new(file)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let ctx = match workspace::find_workspace_root(&project_dir) {
        Some(c) => c,
        None => {
            let fmt = action_format(action);
            if fmt == "json" {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "no_workspace",
                        "searched_from": project_dir.display().to_string(),
                    })
                );
            } else {
                eprintln!(
                    "No [workspace] section found walking up from {}.",
                    project_dir.display()
                );
                eprintln!(
                    "Add a `[workspace] members = [...]` block to a jarvy.toml at the repo root."
                );
            }
            return crate::error_codes::CONFIG_ERROR;
        }
    };

    match action {
        WorkspaceAction::List { output_format } => list(&ctx, output_format),
        WorkspaceAction::Show {
            name,
            output_format,
        } => show(&ctx, name, output_format),
        WorkspaceAction::Validate { output_format } => validate(&ctx, output_format),
    }
}

fn action_format(action: &WorkspaceAction) -> &str {
    match action {
        WorkspaceAction::List { output_format }
        | WorkspaceAction::Show { output_format, .. }
        | WorkspaceAction::Validate { output_format } => output_format.as_str(),
    }
}

fn list(ctx: &workspace::WorkspaceContext, output_format: &str) -> i32 {
    let root_dir = ctx.root_config.parent().unwrap_or(Path::new("."));
    let summaries: Vec<MemberSummary> = ctx
        .workspace
        .members
        .iter()
        .map(|m| collect_member(root_dir, m, ctx))
        .collect();

    if output_format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "workspace_root": root_dir.display().to_string(),
                "inherit": &ctx.workspace.inherit,
                "members": &summaries,
            }))
            .unwrap_or_else(|_| "{}".into())
        );
        return 0;
    }

    println!("Workspace: {}", root_dir.display());
    if !ctx.workspace.inherit.is_empty() {
        println!("Inherits: {}", ctx.workspace.inherit.join(", "));
    }
    println!("Members ({}):", summaries.len());
    for s in &summaries {
        let exists = if s.config_exists { "ok " } else { "MISS" };
        let tools = if s.tools.is_empty() {
            "(no tools)".to_string()
        } else {
            s.tools.keys().cloned().collect::<Vec<_>>().join(", ")
        };
        println!("  [{exists}] {:<22} {tools}", s.name);
    }
    0
}

fn show(ctx: &workspace::WorkspaceContext, name: &str, output_format: &str) -> i32 {
    let root_dir = ctx.root_config.parent().unwrap_or(Path::new("."));
    if !ctx.workspace.members.iter().any(|m| m == name) {
        if output_format == "json" {
            println!(
                "{}",
                serde_json::json!({"status": "unknown_member", "name": name})
            );
        } else {
            eprintln!("Member '{name}' not declared in [workspace] members.");
        }
        return crate::error_codes::CONFIG_ERROR;
    }

    let summary = collect_member(root_dir, name, ctx);

    if output_format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "workspace_root": root_dir.display().to_string(),
                "member": &summary,
            }))
            .unwrap_or_else(|_| "{}".into())
        );
        return 0;
    }

    println!("Project: {}", summary.name);
    println!("Path:    {}", summary.path);
    println!(
        "Config:  {}",
        if summary.config_exists {
            summary.config_path.clone()
        } else {
            format!("{} (missing)", summary.config_path)
        }
    );
    if !ctx.workspace.inherit.is_empty() {
        println!("Inherits sections: {}", ctx.workspace.inherit.join(", "));
    }
    println!();
    println!("Tools ({}):", summary.tools.len());
    for (name, version) in &summary.tools {
        let mark = if summary.overridden.contains(name) {
            " (overridden)"
        } else if summary.inherited.contains(name) {
            " (inherited)"
        } else {
            ""
        };
        println!("  {name} = \"{version}\"{mark}");
    }
    0
}

fn validate(ctx: &workspace::WorkspaceContext, output_format: &str) -> i32 {
    let root_dir = ctx.root_config.parent().unwrap_or(Path::new("."));
    let mut warnings: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let mut entries: Vec<serde_json::Value> = Vec::new();

    for member in &ctx.workspace.members {
        let member_dir = root_dir.join(member);
        let cfg_path = member_dir.join("jarvy.toml");
        let exists = member_dir.is_dir();
        let cfg_exists = cfg_path.exists();
        let parse_ok = if cfg_exists {
            std::fs::read_to_string(&cfg_path)
                .ok()
                .and_then(|s| toml::from_str::<toml::Value>(&s).ok())
                .is_some()
        } else {
            true
        };

        if !exists {
            errors.push(format!("{member}: directory missing"));
        } else if !cfg_exists {
            warnings.push(format!(
                "{member}: no jarvy.toml (workspace defaults apply)"
            ));
        } else if !parse_ok {
            errors.push(format!("{member}: jarvy.toml failed to parse"));
        }

        entries.push(serde_json::json!({
            "name": member,
            "dir_exists": exists,
            "config_exists": cfg_exists,
            "config_parses": parse_ok,
        }));
    }

    if output_format == "json" {
        let status = if !errors.is_empty() {
            "invalid"
        } else if !warnings.is_empty() {
            "warnings"
        } else {
            "ok"
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "status": status,
                "errors": errors,
                "warnings": warnings,
                "members": entries,
            }))
            .unwrap_or_else(|_| "{}".into())
        );
        return if errors.is_empty() {
            0
        } else {
            crate::error_codes::CONFIG_ERROR
        };
    }

    println!("Validating workspace at {}", root_dir.display());
    for line in &warnings {
        println!("  warn: {line}");
    }
    for line in &errors {
        println!("  err:  {line}");
    }
    if errors.is_empty() && warnings.is_empty() {
        println!("  All {} members ok.", ctx.workspace.members.len());
    } else {
        println!(
            "  {} ok, {} warning(s), {} error(s).",
            ctx.workspace.members.len() - warnings.len() - errors.len(),
            warnings.len(),
            errors.len(),
        );
    }
    if errors.is_empty() {
        0
    } else {
        crate::error_codes::CONFIG_ERROR
    }
}

#[derive(serde::Serialize)]
struct MemberSummary {
    name: String,
    path: String,
    config_path: String,
    config_exists: bool,
    tools: BTreeMap<String, String>,
    inherited: Vec<String>,
    overridden: Vec<String>,
}

fn collect_member(
    root_dir: &Path,
    member: &str,
    ctx: &workspace::WorkspaceContext,
) -> MemberSummary {
    let member_dir = root_dir.join(member);
    let cfg_path = member_dir.join("jarvy.toml");
    let config_exists = cfg_path.exists();

    let root_value: toml::Value = std::fs::read_to_string(&ctx.root_config)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_else(|| toml::Value::Table(toml::Table::new()));

    let member_value: toml::Value = if config_exists {
        std::fs::read_to_string(&cfg_path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_else(|| toml::Value::Table(toml::Table::new()))
    } else {
        toml::Value::Table(toml::Table::new())
    };

    let inherit = if ctx.workspace.inherit.is_empty() {
        // No explicit inherit list — every section from root is implicitly
        // available; surface `provisioner` so the resolver merges per-tool.
        vec!["provisioner".to_string()]
    } else {
        ctx.workspace.inherit.clone()
    };
    let merged = workspace::merge_configs(&root_value, &member_value, &inherit);

    let mut tools: BTreeMap<String, String> = BTreeMap::new();
    let mut inherited: Vec<String> = Vec::new();
    let mut overridden: Vec<String> = Vec::new();

    let root_prov: BTreeMap<String, toml::Value> = root_value
        .get("provisioner")
        .and_then(|v| v.as_table())
        .map(|t| t.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    let member_prov: BTreeMap<String, toml::Value> = member_value
        .get("provisioner")
        .and_then(|v| v.as_table())
        .map(|t| t.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();

    if let Some(merged_prov) = merged.get("provisioner").and_then(|v| v.as_table()) {
        for (name, value) in merged_prov {
            tools.insert(name.clone(), value_to_string(value));
            let in_root = root_prov.contains_key(name);
            let in_member = member_prov.contains_key(name);
            if in_root && in_member {
                overridden.push(name.clone());
            } else if in_root && !in_member {
                inherited.push(name.clone());
            }
        }
    }

    MemberSummary {
        name: member.to_string(),
        path: member_dir.display().to_string(),
        config_path: cfg_path.display().to_string(),
        config_exists,
        tools,
        inherited,
        overridden,
    }
}

fn value_to_string(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn setup_workspace(root: &Path) {
        fs::write(
            root.join("jarvy.toml"),
            r#"
[workspace]
members = ["apps/web", "apps/api"]

[provisioner]
git = "latest"
docker = "latest"
"#,
        )
        .unwrap();
        fs::create_dir_all(root.join("apps/web")).unwrap();
        fs::create_dir_all(root.join("apps/api")).unwrap();
        fs::write(
            root.join("apps/web/jarvy.toml"),
            r#"
[provisioner]
node = "20"
docker = "24.0"
"#,
        )
        .unwrap();
        // apps/api intentionally has no jarvy.toml — exercises the
        // "workspace defaults apply" warning path.
    }

    #[test]
    fn list_includes_all_members() {
        let tmp = tempdir().unwrap();
        setup_workspace(tmp.path());
        let ctx = workspace::find_workspace_root(tmp.path()).unwrap();
        // smoke — list returns 0
        assert_eq!(list(&ctx, "pretty"), 0);
    }

    #[test]
    fn show_unknown_member_returns_config_error() {
        let tmp = tempdir().unwrap();
        setup_workspace(tmp.path());
        let ctx = workspace::find_workspace_root(tmp.path()).unwrap();
        let exit = show(&ctx, "ghost", "pretty");
        assert_eq!(exit, crate::error_codes::CONFIG_ERROR);
    }

    #[test]
    fn collect_member_marks_overridden_and_inherited() {
        let tmp = tempdir().unwrap();
        setup_workspace(tmp.path());
        let ctx = workspace::find_workspace_root(tmp.path()).unwrap();
        let summary = collect_member(tmp.path(), "apps/web", &ctx);
        // git: only in root → inherited
        assert!(summary.inherited.contains(&"git".to_string()));
        // docker: in both → overridden
        assert!(summary.overridden.contains(&"docker".to_string()));
        // node: only in member → not inherited, not overridden
        assert!(!summary.inherited.contains(&"node".to_string()));
        assert!(!summary.overridden.contains(&"node".to_string()));
        // Resolved value reflects the override (member wins).
        assert_eq!(summary.tools.get("docker"), Some(&"24.0".to_string()));
    }

    #[test]
    fn validate_flags_missing_member_jarvy_toml_as_warning() {
        let tmp = tempdir().unwrap();
        setup_workspace(tmp.path());
        let ctx = workspace::find_workspace_root(tmp.path()).unwrap();
        // apps/api has no jarvy.toml — should warn, exit 0.
        let exit = validate(&ctx, "pretty");
        assert_eq!(exit, 0);
    }

    #[test]
    fn validate_returns_error_when_member_dir_missing() {
        let tmp = tempdir().unwrap();
        setup_workspace(tmp.path());
        // Add a bogus member that doesn't exist on disk.
        let new_toml = r#"
[workspace]
members = ["apps/web", "apps/api", "apps/ghost"]

[provisioner]
git = "latest"
"#;
        fs::write(tmp.path().join("jarvy.toml"), new_toml).unwrap();
        let ctx = workspace::find_workspace_root(tmp.path()).unwrap();
        let exit = validate(&ctx, "pretty");
        assert_eq!(exit, crate::error_codes::CONFIG_ERROR);
    }
}
