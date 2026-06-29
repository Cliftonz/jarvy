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
        let apply_started = std::time::Instant::now();
        let telemetry_on = crate::observability::telemetry_gate::is_enabled();
        let (new_text, note) = match existing_text {
            Some(ref text) => match generator::merge_into_existing(text, &report) {
                generator::MergeOutcome::Noop => (text.clone(), Some("no new tools to add")),
                generator::MergeOutcome::Merged(s) => (s, None),
                generator::MergeOutcome::BailedToFresh(s) => (
                    s,
                    Some(
                        "existing [provisioner] block couldn't be safely edited; \
                         falling back to a fresh-rendered jarvy.toml (your other sections were preserved \
                         only if they round-tripped through the TOML parser)",
                    ),
                ),
                generator::MergeOutcome::ExistingUnparseable => {
                    if output_format == "json" {
                        println!(
                            "{}",
                            serde_json::json!({
                                "status": "refused",
                                "reason": "existing jarvy.toml is not valid TOML",
                                "path": file,
                            })
                        );
                    } else {
                        eprintln!("Refusing to --apply: existing {} is not valid TOML.", file);
                        eprintln!("Fix the parse error first (or move the file aside) and re-run.");
                    }
                    return crate::error_codes::CONFIG_ERROR;
                }
            },
            None => (generator::render_fresh(&report), None),
        };
        if let Err(e) = atomic_write(Path::new(file), &new_text) {
            eprintln!("Failed to write {}: {}", file, e);
            return crate::error_codes::CONFIG_ERROR;
        }
        if telemetry_on {
            // Operators graph adoption against this event — the whole
            // PRD-044 purpose is onboarding ergonomics.
            tracing::info!(
                event = "discover.applied",
                tools_added = report.required.len(),
                recommended_added = report.recommended.len(),
                already_configured = report.already_configured.len(),
                target = if note == Some("no new tools to add") {
                    "noop"
                } else if matches!(note, Some(s) if s.starts_with("existing [provisioner]")) {
                    "bailed_to_fresh"
                } else {
                    "merged"
                },
                duration_ms = apply_started.elapsed().as_millis() as u64,
            );
        }
        if output_format != "json" {
            println!("\nWrote {}", file);
            if let Some(msg) = note {
                eprintln!("  note: {msg}");
            }
        }
    }

    0
}

/// Atomic write (review item P2 #18): create a sibling temp file,
/// `fsync`-equivalent via Drop, then `rename` over the target. Mid-
/// crash leaves either the old file (rename didn't happen) or the new
/// file (rename succeeded) — never an empty / partial `jarvy.toml`.
fn atomic_write(target: &Path, content: &str) -> std::io::Result<()> {
    use std::io::Write;
    let parent = target.parent().unwrap_or(Path::new("."));
    // tempfile NamedTempFile lives next to the target so rename is on
    // the same filesystem (cross-fs rename would fall back to copy +
    // delete, defeating atomicity).
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.as_file_mut().write_all(content.as_bytes())?;
    tmp.as_file_mut().flush()?;
    tmp.persist(target)
        .map_err(|e| std::io::Error::other(format!("persist failed: {e}")))?;
    Ok(())
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

    /// Review P1 #8 — when the existing jarvy.toml fails to parse,
    /// `--apply` MUST refuse to write and the original file must be
    /// preserved byte-for-byte. Without this guard, a temporarily
    /// broken config would be silently overwritten with every detected
    /// tool (data loss).
    #[test]
    fn apply_refuses_when_existing_toml_unparseable() {
        let tmp = tempdir().unwrap();
        let toml = tmp.path().join("jarvy.toml");
        let bad = "[provisioner\nbroken =";
        fs::write(&toml, bad).unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        let exit = run_discover(toml.to_str().unwrap(), true, false, "pretty");
        assert_eq!(exit, crate::error_codes::CONFIG_ERROR);
        // File must be unchanged.
        assert_eq!(fs::read_to_string(&toml).unwrap(), bad);
    }

    /// Pins the atomic-write contract: a successful apply produces a
    /// fully-formed jarvy.toml — never empty, never partial. We rely
    /// on the tmp+rename pattern; this test is the regression guard.
    #[test]
    fn apply_atomic_write_lands_complete_file() {
        let tmp = tempdir().unwrap();
        let toml = tmp.path().join("jarvy.toml");
        fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        let exit = run_discover(toml.to_str().unwrap(), true, false, "pretty");
        assert_eq!(exit, 0);
        let written = fs::read_to_string(&toml).unwrap();
        // Round-trips through the TOML parser.
        let parsed: toml::Table = written.parse().expect("jarvy.toml must parse");
        assert!(parsed.contains_key("provisioner"));
    }
}
