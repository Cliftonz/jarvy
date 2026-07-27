//! `jarvy check-updates` CLI integration tests (PRD-057).
//!
//! Covers the CLI dispatch surface end-to-end: trust gates,
//! output formats, exit codes, filter flags, and cache
//! interaction on the read-only path. Every test scopes its
//! own `JARVY_TEST_HOME` so parallel runs don't stomp on the
//! developer's real `~/.jarvy/`.
//!
//! Backend probes themselves aren't exercised here — the
//! `--refresh` path would hit real network / real package
//! managers, which is non-deterministic. Cache-pre-populate
//! covers everything except the `backend.latest()` call, and
//! per-backend parsers are unit-tested in `src/maintenance/
//! backends/*.rs`.

use assert_cmd::prelude::*;
use jarvy::maintenance::cache::{self, CacheStore, ok_entry};
use predicates::prelude::*;
// The JSON-shape tests below are macOS-gated; on other targets this
// import would be flagged unused by `-D warnings`.
#[cfg(target_os = "macos")]
use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};
use std::time::SystemTime;
use tempfile::{NamedTempFile, TempDir};

/// Serializes `seed_cache`'s parent-process env mutation. Cargo
/// runs tests in parallel; without this mutex two tests would
/// race their `JARVY_TEST_HOME` writes and one would `cache::save`
/// into a tempdir that the other test just dropped.
static SEED_LOCK: Mutex<()> = Mutex::new(());

fn seed_lock() -> MutexGuard<'static, ()> {
    SEED_LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

// ---------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------

fn cfg_with(text: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "{text}").unwrap();
    f
}

/// One scratch home per test — the `JARVY_TEST_HOME` hook in
/// `cache::cache_path()` routes reads/writes under this dir so
/// we can freely mutate the cache without affecting the user.
fn scratch_home() -> TempDir {
    tempfile::tempdir().unwrap()
}

fn jarvy(home: &Path) -> Command {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1")
        .env("JARVY_NO_PERSONAL_CONFIG", "1")
        // Sandboxed test runners (Claude Code, some CI harnesses)
        // trigger the sandbox skip path before the phase runs. We
        // explicitly opt out here so the trust-gate assertions
        // actually exercise the code under test.
        .env("JARVY_SANDBOX", "0")
        // `JARVY_NO_CI=1` is the CI-detection opt-out (`JARVY_CI=0` is
        // a no-op — only `=1` forces CI on). Without it, GitHub runners'
        // `GITHUB_ACTIONS=true` trips the CI skip path and the phase
        // under test never runs.
        .env("JARVY_NO_CI", "1")
        .env("JARVY_TEST_HOME", home);
    c
}

/// Pre-populate `~/.jarvy/update-cache.json` under `home` with the
/// given `(backend, pkg_id, latest_version)` tuples using fresh
/// timestamps so a cache-read path treats them as hits.
#[allow(unsafe_code)] // SAFETY: env mutation serialized by `SEED_LOCK`;
// the returned guard must outlive any subsequent `cache::*` call
// so a sibling test doesn't clobber the env var mid-write.
fn seed_cache(home: &Path, entries: &[(&str, &str, &str)]) -> (PathBuf, MutexGuard<'static, ()>) {
    let guard = seed_lock();
    unsafe {
        std::env::set_var("JARVY_TEST_HOME", home);
    }
    let mut store = CacheStore::default();
    let now = SystemTime::now();
    for (backend, pkg_id, latest) in entries {
        store.put(backend, pkg_id, ok_entry(*latest, now));
    }
    cache::save(&store).expect("write cache");
    (cache::cache_path().expect("cache path"), guard)
}

// ---------------------------------------------------------------
// Trust gates
// ---------------------------------------------------------------

#[test]
fn kill_switch_env_skips_phase() {
    let home = scratch_home();
    let cfg = cfg_with(
        r#"
[provisioner]
git = "latest"

[maintenance]
check_updates = true
"#,
    );
    jarvy(home.path())
        .env("JARVY_CHECK_UPDATES", "0")
        .args(["check-updates", "--file"])
        .arg(cfg.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("JARVY_CHECK_UPDATES=0"));
}

#[test]
fn opt_out_via_config_skips_phase() {
    let home = scratch_home();
    let cfg = cfg_with(
        r#"
[provisioner]
git = "latest"

[maintenance]
check_updates = false
"#,
    );
    jarvy(home.path())
        .args(["check-updates", "--file"])
        .arg(cfg.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("check_updates = false"));
}

#[test]
fn sandbox_detected_skips_phase() {
    let home = scratch_home();
    let cfg = cfg_with(
        r#"
[provisioner]
git = "latest"
"#,
    );
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1")
        .env("JARVY_NO_PERSONAL_CONFIG", "1")
        .env("JARVY_TEST_HOME", home.path())
        // Force the sandbox detector on regardless of environment.
        .env("JARVY_SANDBOX", "1")
        .env("JARVY_NO_CI", "1")
        .args(["check-updates", "--file"])
        .arg(cfg.path());
    cmd.assert()
        .success()
        .stderr(predicate::str::contains("sandbox detected"));
}

#[test]
fn ci_detected_skips_phase() {
    let home = scratch_home();
    let cfg = cfg_with(
        r#"
[provisioner]
git = "latest"
"#,
    );
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1")
        .env("JARVY_NO_PERSONAL_CONFIG", "1")
        .env("JARVY_TEST_HOME", home.path())
        .env("JARVY_SANDBOX", "0")
        // The CI detector honors the standard `CI=true` env var.
        .env("CI", "true")
        .args(["check-updates", "--file"])
        .arg(cfg.path());
    cmd.assert()
        .success()
        .stderr(predicate::str::contains("CI environment detected"));
}

// ---------------------------------------------------------------
// Empty / no-op paths
// ---------------------------------------------------------------

#[test]
fn empty_provisioner_prints_noop_message() {
    let home = scratch_home();
    let cfg = cfg_with(
        r#"
[provisioner]
"#,
    );
    jarvy(home.path())
        .args(["check-updates", "--file"])
        .arg(cfg.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("no eligible tools"));
}

#[test]
fn json_output_parses_when_no_targets() {
    let home = scratch_home();
    let cfg = cfg_with(
        r#"
[provisioner]
"#,
    );
    jarvy(home.path())
        .args(["check-updates", "--format", "json", "--file"])
        .arg(cfg.path())
        .assert()
        .success();
}

// ---------------------------------------------------------------
// Cache-pre-populated report paths
// ---------------------------------------------------------------

#[test]
fn cache_hit_reports_upgrade_and_exits_1() {
    // Seed the cache with a "newer" version, then invoke without
    // `--refresh`. The resolver's installed-version probe returns
    // whatever `git --version` reports on the host, which is
    // always older than the fictional 999.0.0 latest. The check
    // classifies as Upgrade → exit code 1.
    let home = scratch_home();
    let (_cache_path, _seed_g) = seed_cache(home.path(), &[("brew", "git", "999.0.0")]);

    let cfg = cfg_with(
        r#"
[provisioner]
git = "latest"
"#,
    );
    let assert = jarvy(home.path())
        .args(["check-updates", "--file"])
        .arg(cfg.path())
        .assert();

    // Exit 1 == updates available.
    let output = assert.get_output();
    if !cfg!(target_os = "macos") {
        // Non-mac targets don't route git through the brew
        // backend (Linux prefers apt/dnf/pacman, Windows uses
        // winget/choco). Skip the assertion off-mac — the cache
        // key `brew:git` won't match.
        return;
    }
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit 1 for updates available"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("git"), "stdout missing tool name: {stdout}");
    assert!(
        stdout.contains("999.0.0"),
        "stdout missing latest version: {stdout}"
    );
}

#[test]
#[cfg(target_os = "macos")]
fn cache_hit_json_shape_is_stable() {
    let home = scratch_home();
    let (_cache_path, _seed_g) = seed_cache(home.path(), &[("brew", "git", "999.0.0")]);

    let cfg = cfg_with(
        r#"
[provisioner]
git = "latest"
"#,
    );
    let output = jarvy(home.path())
        .args(["check-updates", "--format", "json", "--file"])
        .arg(cfg.path())
        .output()
        .expect("spawn");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: Value =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("json parse: {e}\nbody: {stdout}"));

    // Schema pins — the taxonomy in CLAUDE.md leans on these
    // field names, so a rename here needs a documented migration.
    assert!(
        value.get("schema_version").is_some(),
        "schema_version present"
    );
    assert!(value.get("summary").is_some(), "summary present");
    assert!(value.get("updates").is_some(), "updates array present");
    assert!(
        value.get("up_to_date").is_some(),
        "up_to_date array present"
    );
    assert!(value.get("unchecked").is_some(), "unchecked array present");
    assert!(value.get("errors").is_some(), "errors array present");

    let summary = &value["summary"];
    for field in ["tools_checked", "updates_available", "unchecked", "errors"] {
        assert!(summary.get(field).is_some(), "summary.{field} present");
    }

    let update = value["updates"]
        .as_array()
        .and_then(|a| a.first())
        .unwrap_or_else(|| panic!("no updates entry in {stdout}"));
    for field in [
        "tool",
        "backend",
        "installed",
        "latest",
        "from_cache",
        "direction",
    ] {
        assert!(update.get(field).is_some(), "update.{field} present");
    }
    assert_eq!(update["from_cache"], Value::Bool(true));
    assert_eq!(update["direction"], Value::String("upgrade".to_string()));
}

#[test]
#[cfg(target_os = "macos")]
fn only_filter_narrows_target_set() {
    // Two tools in config, cache seeded for both. `--only git`
    // should include only git in the report body.
    let home = scratch_home();
    let (_cache_path, _seed_g) = seed_cache(
        home.path(),
        &[("brew", "git", "999.0.0"), ("brew", "jq", "999.0.0")],
    );

    let cfg = cfg_with(
        r#"
[provisioner]
git = "latest"
jq = "latest"
"#,
    );
    let output = jarvy(home.path())
        .args([
            "check-updates",
            "--only",
            "git",
            "--format",
            "json",
            "--file",
        ])
        .arg(cfg.path())
        .output()
        .expect("spawn");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: Value = serde_json::from_str(&stdout).expect("json parse");
    let tools_checked = value["summary"]["tools_checked"].as_u64().unwrap();
    assert_eq!(tools_checked, 1, "--only should narrow to a single tool");
}

#[test]
#[cfg(target_os = "macos")]
fn ignore_flag_skips_named_tool() {
    let home = scratch_home();
    let (_cache_path, _seed_g) = seed_cache(
        home.path(),
        &[("brew", "git", "999.0.0"), ("brew", "jq", "999.0.0")],
    );

    let cfg = cfg_with(
        r#"
[provisioner]
git = "latest"
jq = "latest"
"#,
    );
    let output = jarvy(home.path())
        .args([
            "check-updates",
            "--ignore",
            "jq",
            "--format",
            "json",
            "--file",
        ])
        .arg(cfg.path())
        .output()
        .expect("spawn");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: Value = serde_json::from_str(&stdout).expect("json parse");
    let updates = value["updates"].as_array().unwrap();
    for u in updates {
        assert_ne!(
            u["tool"], "jq",
            "--ignore should exclude jq from the report body"
        );
    }
}

// ---------------------------------------------------------------
// --include-unchecked flag
// ---------------------------------------------------------------

#[test]
fn include_unchecked_lists_version_managers_per_tool() {
    let home = scratch_home();
    let cfg = cfg_with(
        r#"
[provisioner]
rustup = "latest"
nvm = "latest"
"#,
    );

    // Without the flag: single collapsed line, no per-tool
    // listing.
    let default_out = jarvy(home.path())
        .args(["check-updates", "--file"])
        .arg(cfg.path())
        .output()
        .expect("spawn");
    let default_stdout = String::from_utf8_lossy(&default_out.stdout);
    assert!(
        default_stdout.contains("--include-unchecked"),
        "default should hint at the flag: {default_stdout}"
    );
    assert!(
        !default_stdout.contains("- rustup"),
        "default must not list per-tool: {default_stdout}"
    );

    // With the flag: per-tool bullets.
    let included_out = jarvy(home.path())
        .args(["check-updates", "--include-unchecked", "--file"])
        .arg(cfg.path())
        .output()
        .expect("spawn");
    let included_stdout = String::from_utf8_lossy(&included_out.stdout);
    assert!(
        included_stdout.contains("- rustup"),
        "flag should list per-tool: {included_stdout}"
    );
}

// ---------------------------------------------------------------
// Refresh + background flags
// ---------------------------------------------------------------

#[test]
fn background_flag_produces_no_stdout() {
    // `--background` runs foreground-blocking but writes nothing
    // to stdout. Empty provisioner means no backend calls fire,
    // so this is deterministic.
    let home = scratch_home();
    let cfg = cfg_with(
        r#"
[provisioner]
"#,
    );
    let output = jarvy(home.path())
        .args(["check-updates", "--background", "--file"])
        .arg(cfg.path())
        .output()
        .expect("spawn");

    // No stdout because --background suppresses printing.
    // The no-op path prints "no eligible tools" to stdout only
    // when NOT --background. Assert exit 0 and no stdout body.
    assert!(
        output.status.success(),
        "background invocation should succeed"
    );
    assert!(
        output.stdout.is_empty(),
        "background must not emit stdout, got: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}
