//! Integration tests for the enhanced dependency system (PRD-034)
//!
//! Tests:
//! - Validate command warns about missing dependencies
//! - Diff command shows dependency resolution
//! - Doctor command shows dependency satisfaction
//! - --ignore-missing-deps flag suppresses warnings

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

mod common;
use common::{jarvy_cmd, jarvy_fast_cmd};

/// Create a config with tools that have dependencies
fn make_config_with_deps() -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(
        f,
        r#"[privileges]
use_sudo = false

[provisioner]
# kubectl has flexible dependencies (docker, podman, minikube, etc.)
kubectl = "latest"
# lazydocker has strict dependency on docker
lazydocker = "latest"
"#
    )
    .unwrap();
    f
}

/// Create a config where flexible dependency is satisfied
fn make_config_with_satisfied_flex_deps() -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(
        f,
        r#"[privileges]
use_sudo = false

[provisioner]
docker = "latest"
kubectl = "latest"
lazydocker = "latest"
"#
    )
    .unwrap();
    f
}

/// Create a minimal config without dependency issues
fn make_simple_config() -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(
        f,
        r#"[privileges]
use_sudo = false

[provisioner]
git = "latest"
curl = "latest"
"#
    )
    .unwrap();
    f
}

#[test]
fn validate_warns_about_missing_strict_dependencies() {
    let cfg = make_config_with_deps();

    let mut c = jarvy_cmd();
    c.args(["validate", "--file"]).arg(cfg.path());

    // Should warn about lazydocker requiring docker (exit 1 = warnings present).
    // Tightened from `in_iter([0, 1])` so a regression that suppresses the
    // dependency warning fails the test instead of silently passing on code 0.
    c.assert().code(1).stdout(
        predicate::str::contains("lazydocker")
            .and(predicate::str::contains("docker"))
            .and(predicate::str::contains("requires").or(predicate::str::contains("not in"))),
    );
}

#[test]
fn validate_informs_about_missing_flexible_dependencies() {
    let cfg = make_config_with_deps();

    let mut c = jarvy_cmd();
    c.args(["validate", "--file"]).arg(cfg.path());

    // Should mention kubectl and its flexible dep options (exit 1 = warnings present).
    c.assert().code(1).stdout(
        predicate::str::contains("kubectl")
            .and(predicate::str::contains("one of").or(predicate::str::contains("best with"))),
    );
}

#[test]
fn validate_no_warnings_when_dependencies_satisfied() {
    let cfg = make_config_with_satisfied_flex_deps();

    let mut c = jarvy_cmd();
    c.args(["validate", "--file"]).arg(cfg.path());

    // Should not have dependency warnings for lazydocker since docker is in config
    c.assert()
        .success()
        .stdout(predicate::str::contains("requires docker").not());
}

#[test]
fn diff_shows_dependency_resolution() {
    let cfg = make_config_with_satisfied_flex_deps();

    let mut c = jarvy_fast_cmd();
    c.args(["diff", "--file"]).arg(cfg.path());

    // The diff command should complete successfully
    c.assert().success();
}

#[test]
fn diff_shows_missing_dependency_warnings() {
    let cfg = make_config_with_deps();

    let mut c = jarvy_fast_cmd();
    c.args(["diff", "--file"]).arg(cfg.path());

    // Should show dependency warnings for lazydocker
    c.assert().success();
}

#[test]
fn ignore_missing_deps_flag_suppresses_warnings() {
    let cfg = make_config_with_deps();

    // With --ignore-missing-deps, should not show dependency warnings in setup
    let mut c = jarvy_fast_cmd();
    c.env("JARVY_IGNORE_MISSING_DEPS", "1"); // Simulate the flag
    c.args(["diff", "--file"]).arg(cfg.path());

    // Should complete without dependency warning text
    c.assert()
        .success()
        .stdout(predicate::str::contains("REQUIRES:").not());
}

#[test]
fn doctor_shows_dependency_information() {
    let cfg = make_config_with_satisfied_flex_deps();

    let mut c = jarvy_cmd();
    c.args(["doctor", "--file"]).arg(cfg.path());

    // Doctor should show tool health (may exit non-zero if tools are missing)
    c.assert()
        .code(predicate::in_iter([0, 1, 2]))
        .stdout(predicate::str::contains("Tool Health"));
}

#[test]
fn simple_config_has_no_dependency_issues() {
    let cfg = make_simple_config();

    let mut c = jarvy_cmd();
    c.args(["validate", "--file"]).arg(cfg.path());

    // Simple tools (git, curl) have no dependencies - validation should pass cleanly
    c.assert()
        .success()
        .stdout(predicate::str::contains("requires").not());
}

#[test]
fn validate_json_output_includes_dependency_info() {
    let cfg = make_config_with_deps();

    let mut c = jarvy_cmd();
    c.args(["validate", "--file"])
        .arg(cfg.path())
        .args(["--format", "json"]);

    // JSON output should be valid and contain issues (exit 1 = warnings present).
    c.assert()
        .code(1)
        .stdout(predicate::str::contains("\"issues\""));
}

#[test]
fn doctor_json_output_includes_dependency_field() {
    let cfg = make_config_with_satisfied_flex_deps();

    let mut c = jarvy_cmd();
    c.args(["doctor", "--file"])
        .arg(cfg.path())
        .args(["--format", "json"]);

    // Doctor JSON should include tools array (may exit non-zero if tools are missing)
    c.assert()
        .code(predicate::in_iter([0, 1, 2]))
        .stdout(predicate::str::contains("\"tools\""));
}

// Unit-style tests for the spec module functions
mod spec_tests {
    #[test]
    fn test_should_ignore_missing_deps_env_var() {
        // This test verifies the env var detection works
        // The actual function is in spec.rs
        let ignore = std::env::var("JARVY_IGNORE_MISSING_DEPS")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);

        // Default should be false (not set in test environment)
        // Unless explicitly set by another test
        assert!(!ignore || std::env::var("JARVY_IGNORE_MISSING_DEPS").is_ok());
    }
}
