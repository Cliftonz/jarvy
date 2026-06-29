//! `jarvy discover` CLI handler.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::{analyze, generator};

pub fn run_discover(file: &str, apply: bool, missing: bool, output_format: &str) -> i32 {
    let project_dir: PathBuf = Path::new(file)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let existing_text = std::fs::read_to_string(file).ok();
    let already_configured = existing_text
        .as_deref()
        .map(parse_provisioner_keys)
        .unwrap_or_default();

    let known = known_tool_set();
    let report = analyze(&project_dir, &already_configured, &known);

    if output_format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "project_dir": project_dir.display().to_string(),
                "detections": &report.detections,
                "required": &report.required,
                "recommended": &report.recommended,
                "already_configured": &report.already_configured,
                "applied": apply,
            }))
            .unwrap_or_else(|_| "{}".to_string())
        );
    } else if missing {
        render_missing_only(&report);
    } else {
        render_pretty(&report, file, apply);
    }

    if apply {
        let new_text = match existing_text {
            Some(text) => generator::merge_into_existing(&text, &report),
            None => generator::render_fresh(&report),
        };
        if let Err(e) = std::fs::write(file, &new_text) {
            eprintln!("Failed to write {}: {}", file, e);
            return crate::error_codes::CONFIG_ERROR;
        }
        if output_format != "json" {
            println!("\nWrote {}", file);
        }
    }

    0
}

fn render_pretty(report: &super::DiscoverReport, file: &str, apply: bool) {
    println!("Project Analysis");
    println!("================\n");

    if report.detections.is_empty() {
        println!("No supported technologies detected.");
        println!("(Looked for: Cargo.toml, package.json, pyproject.toml, go.mod,");
        println!(" Gemfile, Dockerfile, k8s/, *.tf, Makefile, Justfile, etc.)");
        return;
    }

    println!("Detected Technologies:");
    for d in &report.detections {
        let v = d.version.as_deref().unwrap_or("(version not pinned)");
        println!("  {:<14} {:<12}  (from {})", d.tool, v, d.source);
    }

    if !report.already_configured.is_empty() {
        println!("\nAlready in jarvy.toml:");
        for name in &report.already_configured {
            println!("  {name}");
        }
    }

    if !report.required.is_empty() {
        println!("\nRequired (would be added):");
        for s in &report.required {
            println!("  {} = \"{}\"   # {}", s.name, s.version, s.reason);
        }
    }

    if !report.recommended.is_empty() {
        println!("\nRecommended companions:");
        for s in &report.recommended {
            println!("  {} = \"{}\"   # {}", s.name, s.version, s.reason);
        }
    }

    if !apply {
        println!("\nRun `jarvy discover --apply --file {file}` to update jarvy.toml.");
    }
}

fn render_missing_only(report: &super::DiscoverReport) {
    for s in report.required.iter().chain(report.recommended.iter()) {
        println!("{} = \"{}\"", s.name, s.version);
    }
}

fn parse_provisioner_keys(text: &str) -> HashSet<String> {
    match text.parse::<toml::Table>() {
        Ok(t) => match t.get("provisioner") {
            Some(toml::Value::Table(p)) => p.keys().cloned().collect(),
            _ => HashSet::new(),
        },
        Err(_) => HashSet::new(),
    }
}

fn known_tool_set() -> HashSet<String> {
    // Lazy: register tools on first call so the discover command is
    // self-contained (doesn't rely on caller invoking
    // `tools::register_all` first).
    crate::tools::register_all();
    crate::tools::registry::registered_tool_names()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn parse_provisioner_keys_returns_empty_for_empty_text() {
        assert!(parse_provisioner_keys("").is_empty());
    }

    #[test]
    fn parse_provisioner_keys_lists_pinned_tools() {
        let text = r#"
[provisioner]
git = "latest"
docker = "latest"
"#;
        let keys = parse_provisioner_keys(text);
        assert!(keys.contains("git"));
        assert!(keys.contains("docker"));
    }

    #[test]
    fn run_discover_apply_creates_file_in_empty_dir() {
        let tmp = tempdir().unwrap();
        let toml = tmp.path().join("jarvy.toml");
        fs::write(tmp.path().join("Cargo.toml"), "[package]\nname=\"x\"").unwrap();
        let exit = run_discover(toml.to_str().unwrap(), true, false, "pretty");
        assert_eq!(exit, 0);
        let written = fs::read_to_string(&toml).unwrap();
        assert!(written.contains("[provisioner]"));
        assert!(written.contains("rust ="), "got:\n{written}");
    }
}
