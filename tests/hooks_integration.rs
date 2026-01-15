//! Integration tests for post-install hooks feature
//!
//! Tests the hooks functionality including:
//! - pre_setup hook execution
//! - post_setup hook execution
//! - per-tool post_install hooks
//! - timeout behavior
//! - continue_on_error behavior
//! - --no-hooks flag
//! - --dry-run output

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn create_test_config(dir: &TempDir, content: &str) -> String {
    let config_path = dir.path().join("jarvy.toml");
    fs::write(&config_path, content).expect("Failed to write test config");
    config_path.to_string_lossy().to_string()
}

#[test]
fn test_setup_with_pre_setup_hook_dry_run() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config = create_test_config(
        &temp_dir,
        r#"
[provisioner]
git = "latest"

[hooks]
pre_setup = "echo 'Pre-setup hook executed'"
"#,
    );

    let mut cmd = Command::cargo_bin("jarvy").expect("Failed to find binary");
    cmd.arg("setup")
        .arg("--file")
        .arg(&config)
        .arg("--dry-run")
        .env("JARVY_TEST_MODE", "1");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("[DRY-RUN]"))
        .stdout(predicate::str::contains("pre_setup"));
}

#[test]
fn test_setup_with_post_setup_hook_dry_run() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config = create_test_config(
        &temp_dir,
        r#"
[provisioner]
git = "latest"

[hooks]
post_setup = "echo 'Post-setup hook executed'"
"#,
    );

    let mut cmd = Command::cargo_bin("jarvy").expect("Failed to find binary");
    cmd.arg("setup")
        .arg("--file")
        .arg(&config)
        .arg("--dry-run")
        .env("JARVY_TEST_MODE", "1");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("[DRY-RUN]"))
        .stdout(predicate::str::contains("post_setup"));
}

#[test]
fn test_setup_with_tool_hook_dry_run() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config = create_test_config(
        &temp_dir,
        r#"
[provisioner]
git = "latest"

[hooks.git]
post_install = "echo 'Git installed'"
"#,
    );

    let mut cmd = Command::cargo_bin("jarvy").expect("Failed to find binary");
    cmd.arg("setup")
        .arg("--file")
        .arg(&config)
        .arg("--dry-run")
        .env("JARVY_TEST_MODE", "1");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("[DRY-RUN]"))
        .stdout(predicate::str::contains("git post_install"));
}

#[test]
fn test_setup_no_hooks_flag() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config = create_test_config(
        &temp_dir,
        r#"
[provisioner]
git = "latest"

[hooks]
pre_setup = "echo 'This should not run'"
post_setup = "echo 'This should not run either'"
"#,
    );

    let mut cmd = Command::cargo_bin("jarvy").expect("Failed to find binary");
    cmd.arg("setup")
        .arg("--file")
        .arg(&config)
        .arg("--no-hooks")
        .arg("--dry-run")
        .env("JARVY_TEST_MODE", "1");

    cmd.assert()
        .success()
        // Should not show hook dry-run output when --no-hooks is used
        .stdout(predicate::str::contains("pre_setup").not())
        .stdout(predicate::str::contains("post_setup").not());
}

#[test]
fn test_hooks_config_with_custom_shell_dry_run() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config = create_test_config(
        &temp_dir,
        r#"
[provisioner]
git = "latest"

[hooks]
pre_setup = "echo 'Test'"

[hooks.config]
shell = "bash"
timeout = 60
continue_on_error = true
"#,
    );

    let mut cmd = Command::cargo_bin("jarvy").expect("Failed to find binary");
    cmd.arg("setup")
        .arg("--file")
        .arg(&config)
        .arg("--dry-run")
        .env("JARVY_TEST_MODE", "1");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Shell: bash"))
        .stdout(predicate::str::contains("Timeout: 60s"))
        .stdout(predicate::str::contains("Continue on error: true"));
}

#[test]
fn test_hooks_environment_variables_dry_run() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config = create_test_config(
        &temp_dir,
        r#"
[provisioner]
git = "latest"

[hooks.git]
post_install = "echo $JARVY_TOOL $JARVY_VERSION"
"#,
    );

    let mut cmd = Command::cargo_bin("jarvy").expect("Failed to find binary");
    cmd.arg("setup")
        .arg("--file")
        .arg(&config)
        .arg("--dry-run")
        .env("JARVY_TEST_MODE", "1");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("JARVY_TOOL=git"))
        .stdout(predicate::str::contains("JARVY_VERSION=latest"));
}

#[test]
fn test_hooks_config_defaults() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config = create_test_config(
        &temp_dir,
        r#"
[provisioner]
git = "latest"

[hooks]
pre_setup = "echo 'Test'"
"#,
    );

    let mut cmd = Command::cargo_bin("jarvy").expect("Failed to find binary");
    cmd.arg("setup")
        .arg("--file")
        .arg(&config)
        .arg("--dry-run")
        .env("JARVY_TEST_MODE", "1");

    cmd.assert()
        .success()
        // Default timeout is 300 seconds
        .stdout(predicate::str::contains("Timeout: 300s"))
        // Default continue_on_error is false
        .stdout(predicate::str::contains("Continue on error: false"));
}

#[test]
fn test_multiple_hooks_dry_run() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config = create_test_config(
        &temp_dir,
        r#"
[provisioner]
git = "latest"
docker = "latest"

[hooks]
pre_setup = "echo 'Starting...'"
post_setup = "echo 'Done!'"

[hooks.git]
post_install = "git config --global init.defaultBranch main"

[hooks.docker]
post_install = "docker --version"
"#,
    );

    let mut cmd = Command::cargo_bin("jarvy").expect("Failed to find binary");
    cmd.arg("setup")
        .arg("--file")
        .arg(&config)
        .arg("--dry-run")
        .env("JARVY_TEST_MODE", "1");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("pre_setup"))
        .stdout(predicate::str::contains("git post_install"))
        .stdout(predicate::str::contains("docker post_install"))
        .stdout(predicate::str::contains("post_setup"));
}

#[test]
fn test_no_hooks_configured() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config = create_test_config(
        &temp_dir,
        r#"
[provisioner]
git = "latest"
"#,
    );

    let mut cmd = Command::cargo_bin("jarvy").expect("Failed to find binary");
    cmd.arg("setup")
        .arg("--file")
        .arg(&config)
        .arg("--dry-run")
        .env("JARVY_TEST_MODE", "1");

    // Should succeed without any hook-related output
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Running hook").not());
}

#[test]
fn test_cli_help_shows_hook_flags() {
    let mut cmd = Command::cargo_bin("jarvy").expect("Failed to find binary");
    cmd.arg("setup").arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--no-hooks"))
        .stdout(predicate::str::contains("--dry-run"));
}
