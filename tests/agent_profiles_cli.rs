//! CLI-level integration tests for `jarvy agents profile ...` (PRD-058).
//!
//! Exercises the real binary with `JARVY_HOME` redirected to a tempdir so
//! nothing touches the developer's real agent dirs or profile store.
//! `JARVY_HOME` is a first-class (always-compiled) override in
//! `src/paths.rs` — unlike `JARVY_TEST_HOME` it is NOT gated behind the
//! `test-bypass` feature, so this suite needs no `required-features`
//! entry in Cargo.toml (mirrors `tests/cli_dispatch.rs`).

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn jarvy(home: &Path) -> Command {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_HOME", home);
    c.env("JARVY_TEST_MODE", "1");
    c.env("JARVY_TELEMETRY", "0");
    c.env("JARVY_NO_PERSONAL_CONFIG", "1");
    c.args(["agents", "profile"]);
    c
}

/// Seed `<JARVY_HOME>/agent-profiles/<profile>/<slug>/` directly.
/// (`JARVY_HOME` IS the `.jarvy` dir — `paths::jarvy_home()` returns it
/// verbatim — while `Agent::config_dir()` treats it as `$HOME`, so agent
/// dot-dirs like `.claude` sit beside the store.)
fn seed_snapshot(home: &Path, profile: &str, slug: &str) -> std::path::PathBuf {
    let dir = home.join("agent-profiles").join(profile).join(slug);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("settings.json"), "{}").unwrap();
    dir
}

#[test]
fn init_on_empty_home_creates_default_profile_and_registry() {
    let home = TempDir::new().unwrap();

    jarvy(home.path()).arg("init").assert().success().stdout(
        predicate::str::contains("empty profile 'default'")
            .and(predicate::str::contains("Default profile set to 'default'")),
    );

    let store = home.path().join("agent-profiles");
    assert!(store.join("default").is_dir());
    let registry = store.join("profiles.json");
    assert!(registry.exists());
    let body = std::fs::read_to_string(&registry).unwrap();
    assert!(body.contains("\"default_profile\": \"default\""));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let store_mode = std::fs::metadata(&store).unwrap().permissions().mode() & 0o777;
        assert_eq!(store_mode, 0o700, "store dir must be 0700");
        let reg_mode = std::fs::metadata(&registry).unwrap().permissions().mode() & 0o777;
        assert_eq!(reg_mode, 0o600, "registry must be 0600");
    }
}

#[test]
fn create_then_list_json_shows_profile() {
    let home = TempDir::new().unwrap();

    jarvy(home.path())
        .args(["create", "work"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created empty profile 'work'"));

    jarvy(home.path())
        .args(["list", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"work\""));

    // Duplicate create is a config error (exit 2), not a crash.
    jarvy(home.path())
        .args(["create", "work"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn create_with_traversal_name_is_refused() {
    let home = TempDir::new().unwrap();

    jarvy(home.path())
        .args(["create", "../evil"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("invalid profile name"));

    // Nothing escaped the store: `agent-profiles/../evil` would land
    // directly under JARVY_HOME.
    assert!(!home.path().join("evil").exists());
}

#[test]
fn use_print_env_stdout_carries_only_export_lines() {
    let home = TempDir::new().unwrap();
    jarvy(home.path())
        .args(["create", "work"])
        .assert()
        .success();
    let snap = seed_snapshot(home.path(), "work", "claude-code");

    let output = jarvy(home.path())
        .args(["use", "work", "--agents", "claude-code", "--print-env"])
        .output()
        .unwrap();
    assert!(output.status.success(), "use --print-env must exit 0");

    let stdout = String::from_utf8(output.stdout).unwrap();
    // Purity contract: eval'd by the `jp` shell function, so stdout is
    // ONLY export lines — any human text must ride stderr.
    assert!(!stdout.is_empty());
    for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
        assert!(
            line.starts_with("export "),
            "non-export line leaked to stdout: {line:?}"
        );
    }
    assert!(stdout.contains("export CLAUDE_CONFIG_DIR=\""));
    assert!(stdout.contains(&snap.display().to_string()));
    // Stderr purity: export lines must never appear on stderr (they
    // would be silently discarded by the `jp` eval but signal a bug).
    assert!(
        !String::from_utf8_lossy(&output.stderr)
            .lines()
            .any(|l| l.trim_start().starts_with("export ")),
        "export line leaked to stderr"
    );
}

#[test]
fn use_without_snapshot_is_config_error() {
    let home = TempDir::new().unwrap();
    jarvy(home.path())
        .args(["create", "thin"])
        .assert()
        .success();

    jarvy(home.path())
        .args(["use", "thin", "--agents", "claude-code", "--print-env"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("no snapshot"));
}

#[test]
fn delete_missing_profile_exits_config_error() {
    let home = TempDir::new().unwrap();

    jarvy(home.path())
        .args(["delete", "ghost"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn status_json_parses_and_covers_all_agents() {
    let home = TempDir::new().unwrap();
    jarvy(home.path())
        .args(["create", "work"])
        .assert()
        .success();

    let output = jarvy(home.path())
        .args(["status", "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("status --format json must emit valid JSON");
    assert_eq!(value["profiles"], serde_json::json!(["work"]));
    let agents = value["agents"].as_array().expect("agents array");
    assert_eq!(agents.len(), 6, "one entry per known agent");
    for entry in agents {
        for key in [
            "agent",
            "mechanism",
            "switchable_v1",
            "active_profile",
            "state",
        ] {
            assert!(
                entry.get(key).is_some(),
                "agent entry missing key {key}: {entry}"
            );
        }
    }
}

#[test]
fn use_all_storage_only_agents_is_refused() {
    let home = TempDir::new().unwrap();
    jarvy(home.path())
        .args(["create", "myprofile"])
        .assert()
        .success();
    // windsurf is storage-only in v1 — selecting it alone yields no
    // v1-switchable targets and must exit CONFIG_ERROR (2).
    jarvy(home.path())
        .args(["use", "myprofile", "--agents", "windsurf"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("no v1-switchable"));
}

#[test]
fn use_mixed_storage_and_switchable_agents_succeeds() {
    let home = TempDir::new().unwrap();
    jarvy(home.path())
        .args(["create", "myprofile"])
        .assert()
        .success();
    // Seed a claude-code snapshot so `use` has a target to export.
    seed_snapshot(home.path(), "myprofile", "claude-code");

    let output = jarvy(home.path())
        .args([
            "use",
            "myprofile",
            "--agents",
            "windsurf,claude-code",
            "--print-env",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "use with mixed agents must succeed (exit 0); stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("CLAUDE_CONFIG_DIR"),
        "stdout must carry the claude-code export"
    );
    // windsurf is not yet switchable; the advisory message goes to stderr.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not yet switchable"),
        "stderr must mention that windsurf is not yet switchable; got: {stderr}"
    );
}
