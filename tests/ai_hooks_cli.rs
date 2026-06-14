//! CLI surface tests for `jarvy ai-hooks`.
//!
//! Asserts exit codes for every subcommand. Run via `assert_cmd` against
//! the built binary so we cover the same wiring users hit.

use assert_cmd::cargo::CommandCargoExt;
use assert_cmd::prelude::OutputAssertExt;
use predicates::prelude::*;
use std::fs;
use std::process::Command;

fn write_config(dir: &tempfile::TempDir, body: &str) -> String {
    let p = dir.path().join("jarvy.toml");
    fs::write(&p, body).unwrap();
    p.to_string_lossy().into_owned()
}

fn cmd(file: &str, args: &[&str]) -> Command {
    let mut c = Command::cargo_bin("jarvy").unwrap();
    c.arg("ai-hooks");
    c.arg("--file").arg(file);
    c.args(args);
    c.env("JARVY_TEST_MODE", "1");
    c
}

#[test]
fn list_with_no_ai_hooks_section_returns_0() {
    let dir = tempfile::tempdir().unwrap();
    let f = write_config(&dir, "[provisioner]\ngit = \"latest\"\n");
    cmd(&f, &["list"])
        .assert()
        .success()
        .stderr(predicate::str::contains("No [ai_hooks] section"));
}

#[test]
fn list_library_dumps_every_curated_hook() {
    let dir = tempfile::tempdir().unwrap();
    let f = write_config(&dir, "[provisioner]\ngit = \"latest\"\n");
    let out = cmd(&f, &["list", "--library"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8_lossy(&out);
    for name in [
        "block-rm-rf",
        "block-secrets-commit",
        "audit-log",
        "commit-message-format-guard",
    ] {
        assert!(stdout.contains(name), "missing {name} in `list --library`");
    }
}

#[test]
fn test_unknown_hook_returns_2() {
    let dir = tempfile::tempdir().unwrap();
    let f = write_config(&dir, "[provisioner]\ngit = \"latest\"\n");
    cmd(&f, &["test", "definitely-not-a-real-hook"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("Unknown library hook"));
}

#[test]
fn test_known_hook_dumps_script() {
    let dir = tempfile::tempdir().unwrap();
    let f = write_config(&dir, "[provisioner]\ngit = \"latest\"\n");
    cmd(&f, &["test", "block-rm-rf"])
        .assert()
        .success()
        .stdout(predicate::str::contains("block-rm-rf"))
        .stdout(predicate::str::contains("--- bash ---"));
}

#[test]
fn check_clean_returns_0_with_no_hooks() {
    // No agents/hooks configured — apply is a no-op, check has nothing
    // to verify. Should not error.
    let dir = tempfile::tempdir().unwrap();
    let f = write_config(
        &dir,
        "[provisioner]\ngit = \"latest\"\n\n[ai_hooks]\nagents = []\n",
    );
    cmd(&f, &["check"]).assert().success();
}
