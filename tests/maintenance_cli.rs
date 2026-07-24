//! `jarvy check-updates` CLI smoke tests (PRD-057).
//!
//! Deep backend stubbing lives in follow-up work — this file
//! covers the boring but load-bearing surface: the command
//! parses, the trust gates fire, and JSON / human output paths
//! don't panic on empty configs.

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::io::Write;
use std::process::Command;
use tempfile::NamedTempFile;

fn cfg_with(text: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "{text}").unwrap();
    f
}

fn jarvy() -> Command {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1")
        .env("JARVY_NO_PERSONAL_CONFIG", "1")
        // Sandboxed test runners (Claude Code, some CI harnesses)
        // trigger the sandbox skip path before the phase runs. We
        // explicitly opt out here so the trust-gate assertions
        // actually exercise the code under test.
        .env("JARVY_SANDBOX", "0")
        .env("JARVY_CI", "0")
        // Keep the CLI off the real cache — tests should never
        // stomp on the user's `~/.jarvy/update-cache.json`.
        .env(
            "JARVY_TEST_HOME",
            tempfile::tempdir().unwrap().path().to_str().unwrap(),
        );
    c
}

#[test]
fn kill_switch_env_skips_phase() {
    let cfg = cfg_with(
        r#"
[provisioner]
git = "latest"

[maintenance]
check_updates = true
"#,
    );
    jarvy()
        .env("JARVY_CHECK_UPDATES", "0")
        .args(["check-updates", "--file"])
        .arg(cfg.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("JARVY_CHECK_UPDATES=0"));
}

#[test]
fn opt_out_via_config_skips_phase() {
    let cfg = cfg_with(
        r#"
[provisioner]
git = "latest"

[maintenance]
check_updates = false
"#,
    );
    jarvy()
        .args(["check-updates", "--file"])
        .arg(cfg.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("check_updates = false"));
}

#[test]
fn empty_provisioner_prints_noop_message() {
    let cfg = cfg_with(
        r#"
[provisioner]
"#,
    );
    jarvy()
        .args(["check-updates", "--file"])
        .arg(cfg.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("no eligible tools"));
}

#[test]
fn json_output_parses_when_no_targets() {
    let cfg = cfg_with(
        r#"
[provisioner]
"#,
    );
    // `--format json` on the no-op path still exits 0 — but it
    // prints the human "no eligible tools" line. This test just
    // asserts non-crash + zero exit; JSON on the no-op path is
    // documented as a follow-up.
    jarvy()
        .args(["check-updates", "--format", "json", "--file"])
        .arg(cfg.path())
        .assert()
        .success();
}
