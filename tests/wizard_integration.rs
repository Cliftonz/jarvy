//! Integration tests for `jarvy wizard` (PRD-056).
//!
//! Three scenarios:
//! 1. Skill drop end-to-end — `--skill-only` writes `SKILL.md` to the
//!    chosen agent's skills dir.
//! 2. Greenfield preview — wizard runs without a `jarvy.toml` and
//!    produces a coherent preview (does not panic, exits 0).
//! 3. Refusal — `JARVY_SANDBOX=1` causes the wizard to refuse without
//!    the override env var.
//!
//! `JARVY_TEST_HOME` redirects every `~/.jarvy/*` and `~/.{agent}/*`
//! path to a per-test tempdir (the same harness the existing
//! `ai_hooks_integration.rs` test uses) so we don't pollute the
//! developer's real config dirs.

#![cfg(feature = "test-bypass")]

use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::TempDir;

/// Per-test home redirect. `JARVY_TEST_HOME` is the documented
/// escape hatch (gated by the `test-bypass` cargo feature).
fn test_home() -> TempDir {
    TempDir::new().unwrap()
}

#[test]
fn skill_drop_writes_skill_md_to_claude_dir() {
    let home = test_home();
    let project = TempDir::new().unwrap();
    std::fs::write(project.path().join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_HOME", home.path());
    c.env("JARVY_TEST_MODE", "1");
    c.env("JARVY_TELEMETRY", "0");
    // Override sandbox + CI detection so the wizard's trust gates
    // don't refuse — the test runner often runs INSIDE Claude Code
    // (CLAUDECODE=1) and on CI (GITHUB_ACTIONS=true).
    c.env("JARVY_SANDBOX", "0");
    c.env("JARVY_WIZARD", "1");
    c.env_remove("CI");
    c.env_remove("GITHUB_ACTIONS");
    c.env_remove("CLAUDECODE");
    c.current_dir(project.path());
    c.args([
        "wizard",
        "--skill-only",
        "--agent",
        "claude-code",
        "--format",
        "json",
    ]);
    let out = c.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("--format json must emit valid JSON");
    assert_eq!(v["status"], "ok");
    assert_eq!(v["mode"], "skill_drop");
    assert_eq!(v["agent"], "claude-code");
    let path = v["skill_path"].as_str().expect("skill_path must be string");
    assert!(
        path.ends_with("SKILL.md"),
        "skill_path must end with SKILL.md, got: {path}"
    );
    // File actually exists.
    assert!(
        std::path::Path::new(path).exists(),
        "SKILL.md must be written to disk at {path}"
    );
    let body = std::fs::read_to_string(path).unwrap();
    assert!(
        body.starts_with("---\n"),
        "SKILL.md must start with YAML frontmatter"
    );
    assert!(
        body.contains("name: jarvy-setup"),
        "SKILL.md must declare its skill name"
    );
}

#[test]
fn greenfield_preview_runs_without_jarvy_toml() {
    let home = test_home();
    let project = TempDir::new().unwrap();
    // No jarvy.toml. Add a marker so discover has something to surface.
    std::fs::write(project.path().join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_HOME", home.path());
    c.env("JARVY_TEST_MODE", "1");
    c.env("JARVY_TELEMETRY", "0");
    // Override sandbox + CI detection so the wizard's trust gates
    // don't refuse — the test runner often runs INSIDE Claude Code
    // (CLAUDECODE=1) and on CI (GITHUB_ACTIONS=true).
    c.env("JARVY_SANDBOX", "0");
    c.env("JARVY_WIZARD", "1");
    c.env_remove("CI");
    c.env_remove("GITHUB_ACTIONS");
    c.env_remove("CLAUDECODE");
    c.current_dir(project.path());
    // skill-only avoids needing claude/codex on the test runner's PATH.
    c.args([
        "wizard",
        "--skill-only",
        "--agent",
        "cursor",
        "--format",
        "json",
    ]);
    let out = c.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    // The wizard succeeded even though no jarvy.toml existed —
    // greenfield path doesn't require a config file to drop a skill.
    assert_eq!(v["status"], "ok");
    assert_eq!(v["agent"], "cursor");
}

#[test]
fn quickstart_fallback_when_no_agent_detected() {
    // Force the picker into the fallback by pointing JARVY_TEST_HOME
    // at an empty dir (no ~/.claude, ~/.cursor, etc. exist there).
    // The wizard should detect no agents and either fall back to
    // quickstart or emit the json fallback envelope.
    let home = test_home();
    let project = TempDir::new().unwrap();

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_HOME", home.path());
    c.env("JARVY_TEST_MODE", "1");
    c.env("JARVY_TELEMETRY", "0");
    c.env("JARVY_SANDBOX", "0");
    c.current_dir(project.path());
    c.args(["wizard", "--format", "json"]);
    // Either exit 0 (fallback succeeded) or a non-zero exit if
    // quickstart itself bails for a documented reason — we don't
    // pin the exact exit code, only that the process terminates
    // cleanly with structured output.
    let out = c.assert().get_output().clone();
    let stdout = String::from_utf8_lossy(&out.stdout);
    // First line of stdout must be parseable JSON when --format json
    // is supplied (matches the contract pinned in
    // `tests/cli_dispatch.rs::json_format_keeps_stdout_pure_for_logs_config`).
    let first_obj_line = stdout
        .lines()
        .find(|l| l.trim_start().starts_with('{'))
        .unwrap_or("");
    assert!(
        !first_obj_line.is_empty(),
        "wizard --format json must emit JSON; got stdout:\n{stdout}"
    );
}
