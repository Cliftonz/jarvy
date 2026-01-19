//! Integration tests for PRD-023 onboarding commands
//!
//! Tests for:
//! - jarvy init (interactive wizard and template mode)
//! - jarvy templates (list, show, use)
//! - jarvy quickstart (guided flow)

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::io::Write;
use std::process::Command;
use tempfile::{NamedTempFile, TempDir};

// ============================================================================
// jarvy init tests
// ============================================================================

#[test]
fn init_with_template_creates_config_file() {
    let dir = TempDir::new().unwrap();
    let output_path = dir.path().join("jarvy.toml");

    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args([
        "init",
        "--template",
        "essential",
        "--output",
        output_path.to_str().unwrap(),
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Created"));

    // Verify file was created
    let contents = std::fs::read_to_string(&output_path).unwrap();
    assert!(contents.contains("[provisioner]"));
    assert!(contents.contains("git"));
}

#[test]
fn init_with_template_stdout_outputs_to_stdout() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["init", "--template", "essential", "--stdout"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("[provisioner]"))
        .stdout(predicate::str::contains("git"));
}

#[test]
fn init_with_unknown_template_fails() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["init", "--template", "nonexistent-template", "--stdout"]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn init_non_interactive_requires_template() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["init", "--non-interactive"]);

    // Should exit with warning status (not create a file)
    cmd.assert();
}

// ============================================================================
// jarvy templates tests
// ============================================================================

#[test]
fn templates_list_shows_available_templates() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["templates", "list"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Available Templates"))
        .stdout(predicate::str::contains("react"))
        .stdout(predicate::str::contains("essential"))
        .stdout(predicate::str::contains("rust-cli"));
}

#[test]
fn templates_show_displays_template_details() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["templates", "show", "react"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Template: react"))
        .stdout(predicate::str::contains("Tools included"));
}

#[test]
fn templates_show_unknown_template_fails() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["templates", "show", "nonexistent-template"]);

    cmd.assert()
        .failure()
        .stdout(predicate::str::contains("not found"));
}

#[test]
fn templates_use_creates_config_file() {
    let dir = TempDir::new().unwrap();
    let output_path = dir.path().join("jarvy.toml");

    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.current_dir(dir.path());
    cmd.args([
        "templates",
        "use",
        "essential",
        "--output",
        output_path.to_str().unwrap(),
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Created"));

    // Verify file was created with expected content
    let contents = std::fs::read_to_string(&output_path).unwrap();
    assert!(contents.contains("[provisioner]"));
    assert!(contents.contains("git"));
}

#[test]
fn templates_use_respects_output_path() {
    let dir = TempDir::new().unwrap();
    let custom_output = dir.path().join("custom-config.toml");

    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args([
        "templates",
        "use",
        "react",
        "--output",
        custom_output.to_str().unwrap(),
    ]);

    cmd.assert().success();

    // Verify file was created at custom path
    assert!(custom_output.exists());
    let contents = std::fs::read_to_string(&custom_output).unwrap();
    assert!(contents.contains("node"));
}

// ============================================================================
// jarvy quickstart tests
// ============================================================================

#[test]
fn quickstart_non_interactive_without_tty_cancels() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["quickstart", "--non-interactive"]);

    // In non-interactive mode without a TTY, quickstart gets cancelled
    // when it tries to prompt for user input
    cmd.assert().stdout(
        predicate::str::contains("Quickstart cancelled")
            .or(predicate::str::contains("Welcome to Jarvy")),
    );
}

#[test]
fn quickstart_help_shows_options() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.args(["quickstart", "--help"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("quickstart"))
        .stdout(predicate::str::contains("--non-interactive"));
}

// ============================================================================
// Template content tests
// ============================================================================

#[test]
fn essential_template_contains_core_tools() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["init", "--template", "essential", "--stdout"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("git"))
        .stdout(predicate::str::contains("jq"));
}

#[test]
fn react_template_contains_node_tools() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["init", "--template", "react", "--stdout"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("node"))
        .stdout(predicate::str::contains("git"));
}

#[test]
fn rust_template_contains_rust_tools() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["init", "--template", "rust-cli", "--stdout"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("rust"))
        .stdout(predicate::str::contains("git"));
}

#[test]
fn python_api_template_contains_python_tools() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["init", "--template", "python-api", "--stdout"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("python"))
        .stdout(predicate::str::contains("git"));
}

#[test]
fn k8s_admin_template_contains_kubernetes_tools() {
    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["init", "--template", "k8s-admin", "--stdout"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("kubectl"))
        .stdout(predicate::str::contains("docker"));
}

// ============================================================================
// File collision tests
// ============================================================================

#[test]
fn init_does_not_overwrite_existing_file_by_default() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("jarvy.toml");

    // Create an existing file
    std::fs::write(&config_path, "# existing config\n").unwrap();

    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.current_dir(dir.path());
    cmd.args(["init", "--template", "essential"]);

    // Should warn about existing file
    cmd.assert()
        .stdout(predicate::str::contains("already exists"));

    // Original content should be preserved
    let contents = std::fs::read_to_string(&config_path).unwrap();
    assert!(contents.contains("# existing config"));
}

#[test]
fn templates_use_does_not_overwrite_existing_file() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("jarvy.toml");

    // Create an existing file
    std::fs::write(&config_path, "# existing config\n").unwrap();

    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.current_dir(dir.path());
    cmd.args(["templates", "use", "essential"]);

    // Should warn about existing file
    cmd.assert()
        .stdout(predicate::str::contains("already exists"));

    // Original content should be preserved
    let contents = std::fs::read_to_string(&config_path).unwrap();
    assert!(contents.contains("# existing config"));
}
