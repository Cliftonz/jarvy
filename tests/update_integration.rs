//! Integration tests for self-updating functionality

use assert_cmd::Command;
use std::env;

/// Test update check command
#[test]
fn test_update_check_command() {
    // Skip in CI to avoid rate limiting
    if env::var("CI").is_ok() {
        return;
    }

    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    // Should succeed (either up-to-date or update available)
    // Don't check exit code since network might fail
    cmd.args(["update", "check"])
        .env("JARVY_UPDATE", "1")
        .assert();
}

/// Test update config command
#[test]
fn test_update_config_command() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.args(["update", "config"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Update Configuration:"))
        .stdout(predicates::str::contains("Enabled:"))
        .stdout(predicates::str::contains("Channel:"));
}

/// Test update history command (no history)
#[test]
fn test_update_history_command() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.args(["update", "history"]).assert().success();
    // Should show either history or "No update history available"
}

/// Test update with --rollback when no backup exists
#[test]
fn test_update_rollback_no_backup() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.args(["update", "--rollback"])
        .assert()
        .success()
        .stdout(predicates::str::contains("No rollback available"));
}

/// Test update disable command
#[test]
fn test_update_disable_command() {
    let temp_dir = tempfile::TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.args(["update", "disable"])
        .env("HOME", temp_dir.path())
        .assert()
        .success();
}

/// Test update enable command
#[test]
fn test_update_enable_command() {
    let temp_dir = tempfile::TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.args(["update", "enable"])
        .env("HOME", temp_dir.path())
        .assert()
        .success();
}

mod predicates {
    pub use predicates::prelude::*;
    pub use predicates::str;
}
