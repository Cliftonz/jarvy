//! `jarvy context` — diagnose what jarvy thinks the current execution
//! context is (PRD-047 phase 2). Read-only.
//!
//! Useful as a quick "is auto-context detection going to do what I
//! expect?" check before running `jarvy setup` from inside a monorepo
//! member.

use std::path::Path;

pub fn run_context(file: &str, output_format: &str) -> i32 {
    let project_dir = Path::new(file)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let cwd = std::env::current_dir().ok();

    let workspace_ctx = crate::workspace::find_workspace_root(&project_dir);
    let auto_project = crate::commands::setup_cmd::auto_detect_project(file);

    let resolved_setup_file = match auto_project.as_deref() {
        Some(name) => crate::commands::setup_cmd::resolve_workspace_project(file, name)
            .ok()
            .map(|p| p.display().to_string()),
        None => Some(file.to_string()),
    };

    if output_format == "json" {
        let json = serde_json::json!({
            "cwd": cwd.as_ref().map(|p| p.display().to_string()),
            "config_path_arg": file,
            "workspace": workspace_ctx.as_ref().map(|c| {
                serde_json::json!({
                    "root_config": c.root_config.display().to_string(),
                    "members": c.workspace.resolved_members(c.root_config.parent().unwrap_or(Path::new("."))),
                    "current_member": c.current_member,
                    "inherit": c.workspace.effective_inherit(),
                })
            }),
            "auto_detected_project": auto_project,
            "would_setup_file": resolved_setup_file,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".into())
        );
        return 0;
    }

    println!("Jarvy execution context");
    println!("=======================");
    if let Some(ref cwd) = cwd {
        println!("Working dir:   {}", cwd.display());
    }
    println!("--file arg:    {file}");

    match &workspace_ctx {
        Some(ctx) => {
            let root = ctx.root_config.parent().unwrap_or(Path::new("."));
            println!("Workspace:     {}", root.display());
            println!("Root config:   {}", ctx.root_config.display());
            let members = ctx.workspace.resolved_members(root);
            println!("Members ({}):", members.len());
            for m in &members {
                let marker = if Some(m) == ctx.current_member.as_ref() {
                    "  → "
                } else {
                    "    "
                };
                println!("{marker}{m}");
            }
            match &ctx.current_member {
                Some(m) => println!("Current member: {m}"),
                None => println!("Current member: (none — cwd is at workspace root)"),
            }
        }
        None => {
            println!("Workspace:     (not in one)");
        }
    }

    match auto_project {
        Some(ref name) => {
            println!(
                "\nAuto-context:  `jarvy setup` would scope to `{name}` (override with --project)."
            );
        }
        None => {
            println!("\nAuto-context:  none — `jarvy setup` runs against --file as-is.");
        }
    }
    if let Some(ref f) = resolved_setup_file {
        println!("Resolved setup file: {f}");
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_context_in_non_workspace_returns_zero() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("jarvy.toml"),
            "[provisioner]\ngit = \"latest\"\n",
        )
        .unwrap();
        let exit = run_context(tmp.path().join("jarvy.toml").to_str().unwrap(), "pretty");
        assert_eq!(exit, 0);
    }
}
