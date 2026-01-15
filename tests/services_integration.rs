//! Integration tests for service management features (docker-compose, tilt).

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::io::Write;
use std::process::Command;
use tempfile::TempDir;

/// Helper to run jarvy with test mode enabled
fn jarvy_cmd() -> Command {
    let mut c = Command::cargo_bin("jarvy").unwrap();
    c.env("JARVY_TEST_MODE", "1");
    c
}

// =====================================================================
// Services Command Help Tests
// =====================================================================

#[test]
fn services_help_shows_subcommands() {
    let mut c = jarvy_cmd();
    c.args(["services", "--help"]);

    c.assert()
        .success()
        .stdout(predicate::str::contains("start"))
        .stdout(predicate::str::contains("stop"))
        .stdout(predicate::str::contains("status"))
        .stdout(predicate::str::contains("restart"));
}

#[test]
fn services_start_help_shows_foreground_option() {
    let mut c = jarvy_cmd();
    c.args(["services", "start", "--help"]);

    c.assert()
        .success()
        .stdout(predicate::str::contains("--foreground"));
}

// =====================================================================
// Services Command Error Handling Tests
// =====================================================================

#[test]
fn services_without_enabled_config_shows_error() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("jarvy.toml");

    // Write config without services enabled
    let mut config_file = std::fs::File::create(&config_path).unwrap();
    writeln!(config_file, "[provisioner]").unwrap();
    writeln!(config_file, "git = \"*\"").unwrap();

    let mut c = jarvy_cmd();
    c.arg("services");
    c.arg("-f");
    c.arg(&config_path);
    c.arg("status");

    c.assert()
        .success()
        .stderr(predicate::str::contains("Services are not enabled"));
}

#[test]
fn services_with_enabled_but_no_config_file_shows_error() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("jarvy.toml");

    // Write config with services enabled but no compose/tilt file
    let mut config_file = std::fs::File::create(&config_path).unwrap();
    writeln!(config_file, "[provisioner]").unwrap();
    writeln!(config_file, "git = \"*\"").unwrap();
    writeln!(config_file, "[services]").unwrap();
    writeln!(config_file, "enabled = true").unwrap();

    let mut c = jarvy_cmd();
    c.arg("services");
    c.arg("-f");
    c.arg(&config_path);
    c.arg("status");
    c.current_dir(temp_dir.path());

    c.assert()
        .success()
        .stderr(predicate::str::contains("No service configuration found"));
}

// =====================================================================
// Services Config Detection Tests
// =====================================================================

#[test]
fn services_detects_docker_compose_yml() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("jarvy.toml");

    // Write jarvy config with services enabled
    let mut config_file = std::fs::File::create(&config_path).unwrap();
    writeln!(config_file, "[provisioner]").unwrap();
    writeln!(config_file, "git = \"*\"").unwrap();
    writeln!(config_file, "[services]").unwrap();
    writeln!(config_file, "enabled = true").unwrap();

    // Create a docker-compose.yml file
    let compose_path = temp_dir.path().join("docker-compose.yml");
    let mut compose_file = std::fs::File::create(&compose_path).unwrap();
    writeln!(compose_file, "version: '3'").unwrap();
    writeln!(compose_file, "services:").unwrap();
    writeln!(compose_file, "  app:").unwrap();
    writeln!(compose_file, "    image: alpine").unwrap();

    let mut c = jarvy_cmd();
    c.arg("services");
    c.arg("-f");
    c.arg(&config_path);
    c.arg("status");
    c.current_dir(temp_dir.path());

    // Will show either Docker Compose status or that it's not installed
    c.assert().success().stdout(
        predicate::str::contains("Docker Compose").or(predicate::str::contains("not installed")),
    );
}

#[test]
fn services_detects_tiltfile() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("jarvy.toml");

    // Write jarvy config with services enabled
    let mut config_file = std::fs::File::create(&config_path).unwrap();
    writeln!(config_file, "[provisioner]").unwrap();
    writeln!(config_file, "git = \"*\"").unwrap();
    writeln!(config_file, "[services]").unwrap();
    writeln!(config_file, "enabled = true").unwrap();

    // Create a Tiltfile (no docker-compose to ensure Tilt is detected)
    let tilt_path = temp_dir.path().join("Tiltfile");
    let mut tilt_file = std::fs::File::create(&tilt_path).unwrap();
    writeln!(tilt_file, "# Tiltfile").unwrap();

    let mut c = jarvy_cmd();
    c.arg("services");
    c.arg("-f");
    c.arg(&config_path);
    c.arg("status");
    c.current_dir(temp_dir.path());

    // Will show either Tilt status (stdout) or that it's not installed (stderr)
    let output = c.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("Tilt") || combined.contains("not installed"),
        "Expected Tilt detection or 'not installed' message, got stdout: {}, stderr: {}",
        stdout,
        stderr
    );
}

#[test]
fn services_prioritizes_docker_compose_over_tilt() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("jarvy.toml");

    // Write jarvy config with services enabled
    let mut config_file = std::fs::File::create(&config_path).unwrap();
    writeln!(config_file, "[provisioner]").unwrap();
    writeln!(config_file, "git = \"*\"").unwrap();
    writeln!(config_file, "[services]").unwrap();
    writeln!(config_file, "enabled = true").unwrap();

    // Create both docker-compose.yml and Tiltfile
    let compose_path = temp_dir.path().join("docker-compose.yml");
    let mut compose_file = std::fs::File::create(&compose_path).unwrap();
    writeln!(compose_file, "version: '3'").unwrap();

    let tilt_path = temp_dir.path().join("Tiltfile");
    let mut tilt_file = std::fs::File::create(&tilt_path).unwrap();
    writeln!(tilt_file, "# Tiltfile").unwrap();

    let mut c = jarvy_cmd();
    c.arg("services");
    c.arg("-f");
    c.arg(&config_path);
    c.arg("status");
    c.current_dir(temp_dir.path());

    // Docker Compose should be prioritized
    c.assert().success().stdout(
        predicate::str::contains("Docker Compose").or(predicate::str::contains("not installed")),
    );
}

// =====================================================================
// Services Config Override Tests
// =====================================================================

#[test]
fn services_uses_compose_file_override() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("jarvy.toml");

    // Create a subdirectory for the compose file
    let docker_dir = temp_dir.path().join("docker");
    std::fs::create_dir(&docker_dir).unwrap();

    // Write jarvy config with compose_file override
    let mut config_file = std::fs::File::create(&config_path).unwrap();
    writeln!(config_file, "[provisioner]").unwrap();
    writeln!(config_file, "git = \"*\"").unwrap();
    writeln!(config_file, "[services]").unwrap();
    writeln!(config_file, "enabled = true").unwrap();
    writeln!(config_file, "compose_file = \"docker/compose.yml\"").unwrap();

    // Create compose file in subdirectory
    let compose_path = docker_dir.join("compose.yml");
    let mut compose_file = std::fs::File::create(&compose_path).unwrap();
    writeln!(compose_file, "version: '3'").unwrap();

    let mut c = jarvy_cmd();
    c.arg("services");
    c.arg("-f");
    c.arg(&config_path);
    c.arg("status");
    c.current_dir(temp_dir.path());

    // Should find the compose file in the subdirectory
    c.assert().success().stdout(
        predicate::str::contains("Docker Compose").or(predicate::str::contains("not installed")),
    );
}
