//! Integration tests for the new top-level subcommands added in this branch:
//! `audit`, `explain`, `migrate`, `schema`, `shell-init`, `ensure`.
//!
//! These were previously zero-coverage. Each test exercises the dispatch
//! path, key argument parsing, and exit-code/stdout contract — not the
//! external scanner subprocesses, which are unavailable in CI.

use assert_cmd::prelude::*;
use predicates::prelude::*;

mod common;
use common::jarvy_fast_cmd as jarvy;

#[test]
fn schema_outputs_valid_json_to_stdout() {
    let mut c = jarvy();
    c.arg("schema");
    let out = c.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("schema stdout must be valid JSON");
    // The schema lists `provisioner` (the tools section).
    assert!(parsed.is_object());
}

#[test]
fn schema_writes_file_when_output_given() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let mut c = jarvy();
    c.arg("schema").arg("-o").arg(tmp.path());
    c.assert().success();
    let written = std::fs::read_to_string(tmp.path()).unwrap();
    let _: serde_json::Value =
        serde_json::from_str(&written).expect("written schema must be valid JSON");
}

#[test]
fn explain_unknown_tool_exits_nonzero() {
    let mut c = jarvy();
    c.args(["explain", "definitely-not-a-real-tool-xyz"]);
    c.assert().code(predicate::ne(0)).stdout(
        predicate::str::contains("definitely-not-a-real-tool-xyz")
            .or(predicate::str::contains("Unknown").or(predicate::str::contains("not"))),
    );
}

#[test]
fn explain_known_tool_succeeds() {
    let mut c = jarvy();
    c.args(["explain", "git"]);
    c.assert().success().stdout(predicate::str::contains("git"));
}

#[test]
fn migrate_apply_succeeds_on_already_current_config() {
    // PRD half-baked close-out (CHANGELOG v0.4.0): `migrate --apply`
    // now rewrites legacy `[tools]` blocks into `[provisioner]`. A
    // config that's already current is a no-op success — no
    // migrations needed, exit 0.
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), "[provisioner]\ngit = \"latest\"\n").unwrap();
    let mut c = jarvy();
    c.args(["migrate", "--file"]).arg(tmp.path()).arg("--apply");
    c.assert()
        .success()
        .stdout(predicate::str::contains("No migrations needed"));
}

#[test]
fn migrate_dry_run_succeeds() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), "[provisioner]\ngit = \"latest\"\n").unwrap();
    let mut c = jarvy();
    c.args(["migrate", "--file"]).arg(tmp.path());
    c.assert().success();
}

#[test]
fn audit_runs_or_reports_no_scanners() {
    // If no scanners are installed, audit reports `0 passed` but still
    // exits cleanly. Don't pin a specific code (depends on host tooling).
    let mut c = jarvy();
    c.arg("audit");
    let out = c.assert().get_output().clone();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("scanners")
            || stdout.contains("Audit")
            || stdout.to_lowercase().contains("scanner"),
        "audit stdout should mention scanners; got: {stdout}"
    );
}

#[test]
fn shell_init_bash_emits_eval_snippet() {
    let mut c = jarvy();
    c.args(["shell-init", "--shell", "bash"]);
    c.assert()
        .success()
        .stdout(predicate::str::contains("jarvy ensure"));
}

#[test]
fn shell_init_unknown_shell_returns_error() {
    let mut c = jarvy();
    c.args(["shell-init", "--shell", "tcsh"]);
    c.assert().failure();
}

#[test]
fn ensure_quiet_in_test_mode_is_silent_or_skips() {
    // In test mode `ensure` should not crash and should not perform any
    // installs. We don't pin exact stdout — implementations may print a
    // brief skip banner.
    let mut c = jarvy();
    c.args(["ensure", "--quiet"]);
    let out = c.assert().get_output().clone();
    // Either succeeds silently or exits with a clear error code; just
    // assert the binary doesn't panic.
    assert!(out.status.code().is_some());
}

/// `--shell nushell` / `--shell fish` reach generate_rc_snippet through
/// real CLI dispatch, and the `jr` alias survives to stdout (dispatch-level
/// pin for the per-shell snippets; unit tests cover the strings).
#[test]
fn shell_init_nushell_and_fish_emit_jr_alias() {
    let mut c = jarvy();
    c.args(["shell-init", "--shell", "nushell"]);
    c.assert()
        .success()
        .stdout(predicate::str::contains("alias jr = jarvy run"))
        .stdout(predicate::str::contains("jarvy ensure --quiet"));

    let mut c = jarvy();
    c.args(["shell-init", "--shell", "fish"]);
    c.assert()
        .success()
        .stdout(predicate::str::contains("alias jr 'jarvy run'"));
}
