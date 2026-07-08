//! CLI-level integration tests for `jarvy skills {install,update,remove}`
//! (PRD-049 phase 2). Exercises the binary with `--format json` and a
//! redirected home so nothing touches the developer's real agent dirs.
//!
//! `JARVY_HOME` redirects `Agent::config_dir` (and thus `skills_dir`);
//! `JARVY_TEST_HOME` covers the separate test-bypass surface used by
//! other subsystems (global config, etc.). Gated behind the
//! `test-bypass` cargo feature in Cargo.toml, matching the wizard
//! integration suite.

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use tempfile::{NamedTempFile, TempDir};

fn make_config() -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(
        f,
        r#"[privileges]
use_sudo = false

[provisioner]
git = "1.0.0"
"#
    )
    .unwrap();
    f
}

fn jarvy(home: &Path, cfg: &Path) -> Command {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_HOME", home);
    c.env("JARVY_TEST_HOME", home);
    c.env("JARVY_TEST_MODE", "1");
    c.env("JARVY_TELEMETRY", "0");
    c.arg("skills");
    c.args(["--file", &cfg.display().to_string()]);
    c
}

/// Seed a jarvy-managed skill install under `<home>/.claude/skills/`.
fn seed_skill(home: &Path, name: &str, version: &str) {
    let dir = home.join(".claude/skills").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("SKILL.md"), "# seeded skill\n").unwrap();
    std::fs::write(
        dir.join(".jarvy-skill.json"),
        format!(
            r#"{{"skill":"{name}","version":"{version}","skill_md_sha256":"abc","installed_at":"2026-01-01T00:00:00Z"}}"#
        ),
    )
    .unwrap();
}

#[test]
fn remove_deletes_files_and_reports_json() {
    let home = TempDir::new().unwrap();
    let cfg = make_config();
    std::fs::create_dir_all(home.path().join(".claude")).unwrap();
    seed_skill(home.path(), "test-skill", "1.0.0");

    jarvy(home.path(), cfg.path())
        .args(["remove", "test-skill", "--format", "json"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("\"status\": \"ok\"")
                .and(predicate::str::contains("claude-code")),
        );

    let dir = home.path().join(".claude/skills/test-skill");
    assert!(!dir.join("SKILL.md").exists());
    assert!(!dir.join(".jarvy-skill.json").exists());
}

#[test]
fn remove_absent_skill_is_clean_noop() {
    let home = TempDir::new().unwrap();
    let cfg = make_config();
    std::fs::create_dir_all(home.path().join(".claude")).unwrap();

    // Never installed → exit 0, reported as absent, human message clear.
    jarvy(home.path(), cfg.path())
        .args(["remove", "never-installed"])
        .assert()
        .success()
        .stdout(predicate::str::contains("was not installed"));

    jarvy(home.path(), cfg.path())
        .args(["remove", "never-installed", "--format", "json"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("\"status\": \"ok\"")
                .and(predicate::str::contains("\"absent_agents\"")),
        );
}

#[test]
fn remove_refuses_path_traversal_name() {
    let home = TempDir::new().unwrap();
    let cfg = make_config();
    std::fs::create_dir_all(home.path().join(".claude")).unwrap();

    jarvy(home.path(), cfg.path())
        .args(["remove", "../escape", "--format", "json"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("invalid_name"));
}

#[test]
fn remove_with_no_agents_detected_fails() {
    let home = TempDir::new().unwrap(); // no .claude etc. → zero agents
    let cfg = make_config();

    jarvy(home.path(), cfg.path())
        .args(["remove", "anything", "--format", "json"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("no_agents"));
}

#[test]
fn update_with_no_skills_configured_is_noop() {
    let home = TempDir::new().unwrap();
    let cfg = make_config();
    std::fs::create_dir_all(home.path().join(".claude")).unwrap();

    jarvy(home.path(), cfg.path())
        .args(["update", "--format", "json"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("\"status\": \"noop\"")
                .and(predicate::str::contains("no_skills_configured")),
        );
}

#[test]
fn update_named_unresolvable_skill_fails_with_kind() {
    let home = TempDir::new().unwrap();
    let cfg = make_config();
    std::fs::create_dir_all(home.path().join(".claude")).unwrap();

    // Named skill with no library_sources configured → not_in_library.
    jarvy(home.path(), cfg.path())
        .args(["update", "ghost-skill", "--format", "json"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("not_in_library"));
}

#[test]
fn adhoc_install_unresolvable_skill_fails_with_kind() {
    let home = TempDir::new().unwrap();
    let cfg = make_config();
    std::fs::create_dir_all(home.path().join(".claude")).unwrap();

    // Ad-hoc name not in [skills.install] and no library_sources —
    // surfaces the library resolution error rather than the old
    // "not found in [skills.install]" config error.
    jarvy(home.path(), cfg.path())
        .args(["install", "ghost-skill", "--format", "json"])
        .assert()
        .failure()
        .stdout(
            predicate::str::contains("not_in_library")
                .and(predicate::str::contains("\"status\": \"failed\"")),
        );
}

#[test]
fn install_with_no_skills_configured_is_noop() {
    let home = TempDir::new().unwrap();
    let cfg = make_config();
    std::fs::create_dir_all(home.path().join(".claude")).unwrap();

    jarvy(home.path(), cfg.path())
        .args(["install", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"noop\""));
}
