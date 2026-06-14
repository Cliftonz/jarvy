//! Examples regression — every `examples/**/jarvy.toml` must validate
//! cleanly against `jarvy validate` (no Errors, no Unknown-section
//! warnings) and every dotnet example with a `[nuget]` block must
//! surface the full preview in `jarvy setup --dry-run`.
//!
//! Catches three classes of bug:
//! 1. A new top-level config section is added to `Config` but missed
//!    in `validate::TOP_LEVEL_SECTIONS` (covered by `error_count`
//!    assertion plus the Unknown-section message check).
//! 2. A typo in an example references an unknown tool (`gitt`,
//!    `grpculr`) — the validator emits `Severity::Error`, which
//!    bumps `error_count`.
//! 3. The dry-run NuGet branch silently drops a tool from the
//!    preview (covered by the count + per-name assertion).

use std::path::PathBuf;

mod common;
use common::jarvy_fast_cmd as jarvy;

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples")
}

fn discover_example_configs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let root = examples_dir();
    let read = std::fs::read_dir(&root).expect("read examples dir");
    for entry in read.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let jarvy_toml = path.join("jarvy.toml");
        if jarvy_toml.is_file() {
            out.push(jarvy_toml);
        }
    }
    out.sort();
    out
}

/// Discover every dotnet example with a `[nuget]` table. Used to
/// parametrize the dry-run NuGet assertion over all 5 dotnet examples
/// (not just `dotnet-api`).
fn discover_dotnet_examples() -> Vec<PathBuf> {
    discover_example_configs()
        .into_iter()
        .filter(|p| {
            p.parent()
                .and_then(|d| d.file_name())
                .map(|f| f.to_string_lossy().starts_with("dotnet-"))
                .unwrap_or(false)
        })
        .collect()
}

#[test]
fn every_example_validates_without_unknown_section_warnings() {
    let configs = discover_example_configs();
    assert!(
        !configs.is_empty(),
        "expected at least one examples/<name>/jarvy.toml on disk"
    );

    let mut failures: Vec<String> = Vec::new();
    for config in &configs {
        let out = jarvy()
            .args([
                "validate",
                "--file",
                config.to_str().unwrap(),
                "--format",
                "json",
            ])
            .output()
            .expect("run jarvy validate");

        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
            Ok(v) => v,
            Err(e) => {
                failures.push(format!(
                    "{}: validate stdout was not JSON: {} — stdout: {}",
                    config.display(),
                    e,
                    stdout
                ));
                continue;
            }
        };

        // Defense layer 1: zero Severity::Error issues. This catches
        // Unknown-tool ("gitt"), Refused-package-spec ("\x1b[2J"), and
        // anything else the validator escalates to Error.
        let error_count = parsed
            .get("error_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if error_count > 0 {
            let messages: Vec<String> = parsed
                .get("issues")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter(|i| i.get("severity").and_then(|s| s.as_str()) == Some("error"))
                .filter_map(|i| {
                    i.get("message")
                        .and_then(|m| m.as_str())
                        .map(str::to_string)
                })
                .collect();
            failures.push(format!(
                "{}: {} validator error(s):\n      - {}",
                config.display(),
                error_count,
                messages.join("\n      - ")
            ));
        }

        // Defense layer 2: still surface Unknown-section warnings as
        // failures even though they're Warnings (not Errors) by default.
        // The whole point of this test is to prevent the validator
        // shadow-list from drifting against `Config`.
        let issues = parsed
            .get("issues")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for issue in &issues {
            let message = issue
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or_default();
            if message.starts_with("Unknown configuration section") {
                failures.push(format!(
                    "{}: validator warned: {}",
                    config.display(),
                    message
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "examples produced validator issues:\n  - {}",
        failures.join("\n  - ")
    );
}

#[test]
fn dry_run_surfaces_full_nuget_phase_for_every_dotnet_example() {
    let examples = discover_dotnet_examples();
    assert_eq!(
        examples.len(),
        5,
        "expected 5 dotnet-* examples, found {}: {:?}",
        examples.len(),
        examples
    );

    for example in &examples {
        // Parse the example's [nuget] table directly so we can pin the
        // exact tool count and tool names — no approximate `contains`
        // checks on overlapping branches.
        let toml_text = std::fs::read_to_string(example).expect("read example");
        let parsed: toml::Value = toml::from_str(&toml_text).expect("parse example");
        let nuget_table = parsed
            .get("nuget")
            .and_then(|v| v.as_table())
            .unwrap_or_else(|| panic!("{}: expected [nuget] table", example.display()));
        let expected_count = nuget_table.len();
        let expected_names: Vec<&str> = nuget_table.keys().map(String::as_str).collect();

        let out = jarvy()
            .args(["setup", "--dry-run", "--file", example.to_str().unwrap()])
            .output()
            .expect("run jarvy setup --dry-run");
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();

        // Pin the canonical announcement verbatim (substring match
        // because surrounding output may include other phases).
        let expected_announcement = format!(
            "[DRY-RUN] Would install {} .NET global tool(s) via `dotnet tool update -g` (machine-global)",
            expected_count
        );
        assert!(
            stdout.contains(&expected_announcement),
            "{}: dry-run missing announcement `{}`.\n--- stdout ---\n{}",
            example.display(),
            expected_announcement,
            stdout
        );

        // Every key in [nuget] must appear on its own indented preview
        // line. Catches the silent-drop case (HashMap collision, serde
        // rename bug, future "skip duplicates" pass).
        for name in &expected_names {
            let expected_line = format!("[DRY-RUN]   - {}", name);
            assert!(
                stdout.contains(&expected_line),
                "{}: dry-run missing tool line `{}`.\n--- stdout ---\n{}",
                example.display(),
                expected_line,
                stdout
            );
        }
    }
}
