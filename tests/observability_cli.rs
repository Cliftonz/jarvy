//! Integration tests for the observability CLI surface (PRD-027 T16).
//!
//! Exercises the flags end-to-end through the real binary:
//! `doctor --check` category filtering + `--format json`, `doctor
//! --extended`, `setup --profile` / `--profile-output`, `setup
//! --log-format json`, and `diagnose <tool>` / `diagnose --export`.
//!
//! Everything runs in dry-run / test mode with `HOME` redirected to a
//! tempdir, so no host state is touched.

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::io::Write;
use std::process::Command;
use tempfile::{NamedTempFile, TempDir};

/// A jarvy binary invocation with the noise sources (interactive prompts,
/// seamless-mode banner, telemetry) disabled and `HOME` isolated.
fn jarvy(home: &TempDir) -> Command {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1")
        .env("JARVY_SANDBOX", "0")
        .env("JARVY_TELEMETRY", "0")
        .env("JARVY_FAST_TEST", "1")
        .env("HOME", home.path());
    c
}

fn minimal_config() -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(
        f,
        r#"[privileges]
use_sudo = false

[provisioner]
git = "1.0.0"
"#
    )
    .unwrap();
    f
}

// ===== doctor --check category filtering =====

#[test]
fn doctor_check_tools_shows_only_tool_health() {
    let home = TempDir::new().unwrap();
    let out = jarvy(&home)
        .args(["doctor", "--check", "tools", "--tools", "git"])
        .assert()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    // System info is always the context header; Tool Health is selected;
    // PATH Analysis and Hooks Status must be absent.
    assert!(s.contains("System Information"), "system header expected");
    assert!(s.contains("Tool Health"), "tool section expected");
    assert!(
        !s.contains("PATH Analysis"),
        "PATH section must be filtered out:\n{s}"
    );
    assert!(
        !s.contains("Hooks Status"),
        "Hooks section must be filtered out:\n{s}"
    );
}

#[test]
fn doctor_check_path_hooks_excludes_tools() {
    let home = TempDir::new().unwrap();
    let out = jarvy(&home)
        .args(["doctor", "--check", "path,hooks"])
        .assert()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("PATH Analysis"), "PATH section expected");
    assert!(
        !s.contains("Tool Health"),
        "Tool section must be filtered out:\n{s}"
    );
}

#[test]
fn doctor_check_unknown_category_errors() {
    let home = TempDir::new().unwrap();
    jarvy(&home)
        .args(["doctor", "--check", "bogus"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("unknown doctor category"));
}

// ===== doctor output formats =====

#[test]
fn doctor_format_json_is_valid_json() {
    let home = TempDir::new().unwrap();
    let out = jarvy(&home)
        .args(["doctor", "--tools", "git", "--format", "json"])
        .assert()
        .get_output()
        .stdout
        .clone();
    let parsed: serde_json::Value =
        serde_json::from_slice(&out).expect("doctor --format json must emit valid JSON");
    assert!(
        parsed.get("system").is_some() && parsed.get("tools").is_some(),
        "doctor JSON should carry system + tools keys"
    );
}

#[test]
fn doctor_extended_runs() {
    let home = TempDir::new().unwrap();
    let out = jarvy(&home)
        .args(["doctor", "--extended", "--tools", "git"])
        .assert()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("System Metrics") || s.contains("Tool Summary"));
}

// ===== setup --profile =====

#[test]
fn setup_profile_emits_report_to_stderr() {
    let home = TempDir::new().unwrap();
    let cfg = minimal_config();
    let out = jarvy(&home)
        .args(["setup", "--dry-run", "--profile", "--file"])
        .arg(cfg.path())
        .assert()
        .get_output()
        .stderr
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(
        s.contains("Performance Profile"),
        "profile summary expected on stderr:\n{s}"
    );
    assert!(
        s.contains("version_check"),
        "at least one phase name expected:\n{s}"
    );
}

#[test]
fn setup_profile_output_writes_ms_json() {
    let home = TempDir::new().unwrap();
    let cfg = minimal_config();
    let profile_out = home.path().join("profile.json");
    jarvy(&home)
        .args(["setup", "--dry-run", "--profile", "--profile-output"])
        .arg(&profile_out)
        .args(["--file"])
        .arg(cfg.path())
        .assert()
        .success();
    let body = std::fs::read_to_string(&profile_out).expect("profile-output file written");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("profile JSON valid");
    assert!(
        parsed.get("total_duration_ms").is_some(),
        "durations must serialize as integer *_ms, got: {body}"
    );
    assert!(!body.contains("nanos"), "no raw Duration encoding: {body}");
}

#[test]
fn setup_log_format_json_still_succeeds() {
    let home = TempDir::new().unwrap();
    let cfg = minimal_config();
    // --log-format json switches the console layer to JSON; the command
    // must still complete (the flag previously parsed but did nothing).
    jarvy(&home)
        .args(["setup", "--dry-run", "--log-format", "json", "--file"])
        .arg(cfg.path())
        .assert()
        .success();
}

// ===== diagnose =====

#[test]
fn diagnose_known_tool_runs() {
    let home = TempDir::new().unwrap();
    let out = jarvy(&home)
        .args(["diagnose", "git"])
        .assert()
        .get_output()
        .stdout
        .clone();
    assert!(String::from_utf8_lossy(&out).contains("Diagnosing: git"));
}

#[test]
fn diagnose_export_writes_json_report() {
    let home = TempDir::new().unwrap();
    // --export writes jarvy-diagnose-<tool>-<ts>.json into the cwd.
    let workdir = TempDir::new().unwrap();
    jarvy(&home)
        .current_dir(workdir.path())
        .args(["diagnose", "git", "--export"])
        .assert()
        .success();
    let written: Vec<_> = std::fs::read_dir(workdir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| {
            let n = e.file_name();
            let n = n.to_string_lossy();
            n.starts_with("jarvy-diagnose-git-") && n.ends_with(".json")
        })
        .collect();
    assert_eq!(
        written.len(),
        1,
        "exactly one diagnose export file expected"
    );
    let body = std::fs::read_to_string(written[0].path()).unwrap();
    serde_json::from_str::<serde_json::Value>(&body).expect("diagnose export is valid JSON");
}
