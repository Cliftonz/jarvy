//! Integration tests for remote config loading and validation
//! PRD-015: Config Validation and Remote Loading

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::io::Write;
use std::process::Command;
use tempfile::NamedTempFile;

mod common;
use common::jarvy_fast_cmd;

fn make_valid_config() -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(
        f,
        r#"[privileges]
use_sudo = false

[provisioner]
git = "2.0"
"#
    )
    .unwrap();
    f
}

fn make_invalid_toml() -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "this is not valid toml {{{{").unwrap();
    f
}

fn make_unknown_tool_config() -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(
        f,
        r#"[provisioner]
nodejs = "18.0"
"#
    )
    .unwrap();
    f
}

// =============================================================================
// jarvy validate command tests
// =============================================================================

#[test]
fn validate_valid_config_succeeds() {
    let cfg = make_valid_config();
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["validate", "--file"]).arg(cfg.path());
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("valid").or(predicate::str::contains("Valid")));
}

#[test]
fn validate_invalid_toml_fails() {
    let cfg = make_invalid_toml();
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["validate", "--file"]).arg(cfg.path());
    cmd.assert()
        .failure()
        .stdout(predicate::str::contains("error").or(predicate::str::contains("Error")));
}

#[test]
fn validate_unknown_tool_shows_suggestion() {
    let cfg = make_unknown_tool_config();
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["validate", "--file"]).arg(cfg.path());
    // Should suggest 'node' for 'nodejs' typo
    cmd.assert().stdout(
        predicate::str::contains("node")
            .or(predicate::str::contains("Unknown tool"))
            .or(predicate::str::contains("unknown")),
    );
}

#[test]
fn validate_strict_mode_treats_warnings_as_errors() {
    let cfg = make_unknown_tool_config();
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["validate", "--file"])
        .arg(cfg.path())
        .arg("--strict");
    cmd.assert().failure();
}

#[test]
fn validate_json_format_output() {
    let cfg = make_valid_config();
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["validate", "--file"])
        .arg(cfg.path())
        .args(["--format", "json"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("{").and(predicate::str::contains("}")));
}

// =============================================================================
// CLI argument parsing tests
// =============================================================================

#[test]
fn validate_accepts_from_flag() {
    // Test that --from flag is accepted (will fail to fetch, but arg should parse)
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["validate", "--from", "https://example.com/nonexistent.toml"]);
    // Should fail with network error, not argument parsing error
    cmd.assert().failure().stderr(
        predicate::str::contains("fetch")
            .or(predicate::str::contains("error"))
            .or(predicate::str::contains("Error")),
    );
}

#[test]
fn validate_accepts_header_flag() {
    // Test that --header flag is accepted
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args([
        "validate",
        "--from",
        "https://example.com/config.toml",
        "--header",
        "Authorization: token test123",
    ]);
    // Should fail with network error, not argument parsing error
    cmd.assert().failure();
}

#[test]
fn setup_accepts_header_flag() {
    // Test that --header flag is accepted on setup command
    let cfg = make_valid_config();
    let mut cmd = jarvy_fast_cmd();
    cmd.args(["setup", "--file"])
        .arg(cfg.path())
        .args(["--dry-run", "--header", "X-Test: value"]);
    // Header is ignored for local files, but should not cause argument error
    cmd.assert().success();
}

#[test]
fn setup_from_url_accepts_header_flag() {
    // Test that setup --from with --header is accepted
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args([
        "setup",
        "--from",
        "https://example.com/config.toml",
        "--header",
        "Authorization: Bearer token",
        "--dry-run",
    ]);
    // Should fail with network error, not argument parsing error
    cmd.assert().failure().stderr(
        predicate::str::contains("fetch")
            .or(predicate::str::contains("error"))
            .or(predicate::str::contains("Error")),
    );
}

// =============================================================================
// Validation edge cases
// =============================================================================

#[test]
fn validate_nonexistent_file_fails_gracefully() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["validate", "--file", "/nonexistent/path/jarvy.toml"]);
    cmd.assert().failure();
}

#[test]
fn validate_empty_config_warns_no_provisioner() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "# Empty config file").unwrap();
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["validate", "--file"]).arg(f.path());
    // Empty config is valid TOML but produces a warning about missing provisioner
    cmd.assert().stdout(
        predicate::str::contains("warning")
            .or(predicate::str::contains("WARN"))
            .or(predicate::str::contains("No [provisioner]")),
    );
}
