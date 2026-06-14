//! Examples regression — every `examples/**/jarvy.toml` must validate
//! cleanly against `jarvy validate` (no Unknown-section warnings) and the
//! `[nuget]` examples must surface in `jarvy setup --dry-run`.
//!
//! Catches the class of bug where a new top-level config section
//! (`[nuget]` here) is added to the parser + setup pipeline but missed in
//! `src/commands/validate.rs::known_keys` or in the dry-run preview —
//! exactly the two findings the Codex adversarial review raised against
//! commit ec82f49.

use assert_cmd::cargo::cargo_bin;
use std::path::PathBuf;
use std::process::Command;

fn jarvy() -> Command {
    let mut c = Command::new(cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.env("JARVY_FAST_TEST", "1");
    c
}

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
        // `jarvy validate -F json` prints the ValidationResult as JSON on
        // stdout, even when issues are present (exit code reflects severity).
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
        "examples produced Unknown-section warnings:\n  - {}",
        failures.join("\n  - ")
    );
}

#[test]
fn dry_run_surfaces_nuget_phase_for_dotnet_example() {
    let example = examples_dir().join("dotnet-api").join("jarvy.toml");
    assert!(
        example.is_file(),
        "expected examples/dotnet-api/jarvy.toml to exist"
    );

    let out = jarvy()
        .args(["setup", "--dry-run", "--file", example.to_str().unwrap()])
        .output()
        .expect("run jarvy setup --dry-run");

    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let combined = format!("{stdout}\n---STDERR---\n{stderr}");

    // The dry-run should announce the .NET global tools phase so the
    // operator knows machine-global `dotnet tool update -g` calls will
    // run. Without this branch, the .NET examples silently hid a real
    // machine mutation behind a clean preview.
    assert!(
        combined.contains(".NET global tool")
            || combined.contains("dotnet tool update")
            || combined.contains("Would install") && combined.contains("global tool"),
        "dry-run output missing NuGet phase announcement.\n{combined}"
    );

    // And the tool names from the example config should appear in the
    // preview so the operator can review what would land in
    // `~/.dotnet/tools/`.
    assert!(
        combined.contains("dotnet-ef"),
        "dry-run output missing dotnet-ef line.\n{combined}"
    );
    assert!(
        combined.contains("csharpier"),
        "dry-run output missing csharpier line.\n{combined}"
    );
}
