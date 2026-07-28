//! E2E scenario for `jarvy agents profile ...` (PRD-058), run by the
//! cross-platform workflow (`.github/workflows/e2e-cross-platform.yml`)
//! against the RELEASE binary via `JARVY_BIN` — release builds compile
//! out the `test-bypass` feature, so this suite doubles as proof that
//! the always-compiled `JARVY_HOME` override and the whole profile
//! lifecycle work in the shipped artifact on every OS (including the
//! Windows junction fallback for the symlink tier).
//!
//! Hermetic: `JARVY_HOME` is a tempdir that acts as BOTH the `.jarvy`
//! store root and the agent `$HOME` (`Agent::config_dir()` puts
//! `.claude` / `.cursor` beside the store), so nothing touches the
//! runner's real agent dirs. Falls back to the debug binary for local
//! `cargo test --test e2e_agent_profiles` runs.

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn jarvy_bin() -> PathBuf {
    match std::env::var("JARVY_BIN") {
        Ok(bin) => PathBuf::from(bin),
        Err(_) => assert_cmd::cargo::cargo_bin!("jarvy").to_path_buf(),
    }
}

fn profile_cmd(home: &Path, args: &[&str]) -> std::process::Output {
    let mut c = Command::new(jarvy_bin());
    c.env("JARVY_HOME", home);
    c.env("JARVY_TEST_MODE", "1");
    c.env("JARVY_TELEMETRY", "0");
    c.env("JARVY_NO_PERSONAL_CONFIG", "1");
    c.args(["agents", "profile"]);
    c.args(args);
    c.output().expect("failed to spawn jarvy")
}

fn assert_success(out: &std::process::Output, ctx: &str) {
    assert!(
        out.status.success(),
        "{ctx} failed (exit {:?})\nstdout: {}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

/// Canonical target of the live cursor link. `fs::canonicalize` follows
/// symlinks AND Windows junctions uniformly, and normalizes the macOS
/// `/tmp` → `/private/tmp` alias on both sides of the comparison.
fn cursor_resolves_to(home: &Path, snapshot: &Path) -> bool {
    let live = home.join(".cursor");
    match (live.canonicalize(), snapshot.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Remove the live cursor link whatever it is on this OS: Unix symlink
/// (remove_file) or Windows junction / directory symlink (remove_dir).
fn remove_link(link: &Path) {
    if std::fs::remove_file(link).is_err() {
        std::fs::remove_dir(link).expect("remove cursor link");
    }
}

#[test]
fn full_profile_lifecycle_cross_platform() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path();
    let store = home.join("agent-profiles");

    // Pre-seed live agent dirs so `init` has something to snapshot:
    // claude-code exercises the env tier (copy), cursor the symlink
    // tier (move + link-back).
    let claude_live = home.join(".claude");
    std::fs::create_dir_all(&claude_live).unwrap();
    std::fs::write(claude_live.join("settings.json"), "{}").unwrap();
    // codex must exist too: `use --global` without `--agents` switches
    // every v1-switchable agent and refuses if one lacks a snapshot.
    let codex_live = home.join(".codex");
    std::fs::create_dir_all(&codex_live).unwrap();
    std::fs::write(codex_live.join("config.toml"), "").unwrap();
    let cursor_live = home.join(".cursor");
    std::fs::create_dir_all(&cursor_live).unwrap();
    std::fs::write(cursor_live.join("sentinel.txt"), "cursor-config").unwrap();

    // ---- init: snapshot installed agents as 'default' ----
    let out = profile_cmd(home, &["init"]);
    assert_success(&out, "init");

    assert!(
        store.join("default").join("claude-code").is_dir(),
        "claude-code snapshot missing from default profile"
    );
    assert!(
        claude_live.is_dir()
            && !std::fs::symlink_metadata(&claude_live)
                .unwrap()
                .is_symlink(),
        "env-tier live dir must stay a real directory (copy, not move)"
    );
    let default_cursor = store.join("default").join("cursor");
    assert!(
        cursor_resolves_to(home, &default_cursor),
        "cursor must be a link into the default snapshot after init"
    );
    assert_eq!(
        std::fs::read_to_string(cursor_live.join("sentinel.txt")).unwrap(),
        "cursor-config",
        "cursor config must remain readable through the link"
    );

    // ---- create work --from default ----
    let out = profile_cmd(home, &["create", "work", "--from", "default"]);
    assert_success(&out, "create work --from default");
    let work_cursor = store.join("work").join("cursor");
    assert!(
        work_cursor.join("sentinel.txt").exists(),
        "create --from must copy the cursor snapshot"
    );

    // ---- use work --print-env: stdout purity (eval'd by `jp`) ----
    let out = profile_cmd(
        home,
        &["use", "work", "--agents", "claude-code", "--print-env"],
    );
    assert_success(&out, "use work --print-env");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(!stdout.trim().is_empty(), "print-env produced no exports");
    for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
        assert!(
            line.starts_with("export "),
            "non-export line leaked to stdout: {line:?}"
        );
    }
    assert!(stdout.contains("CLAUDE_CONFIG_DIR"));
    assert!(stdout.contains("agent-profiles"));

    // ---- use work --global: cursor link re-points atomically ----
    let out = profile_cmd(home, &["use", "work", "--global"]);
    assert_success(&out, "use work --global");
    assert!(
        cursor_resolves_to(home, &work_cursor),
        "cursor must resolve into the work snapshot after --global switch"
    );

    // ---- status --format json: parses, cursor is managed ----
    let out = profile_cmd(home, &["status", "--format", "json"]);
    assert_success(&out, "status --format json");
    let value: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("status must emit valid JSON");
    let agents = value["agents"].as_array().expect("agents array");
    let cursor = agents
        .iter()
        .find(|a| a["agent"] == "cursor")
        .expect("cursor entry in status");
    assert_eq!(cursor["state"], "managed");
    assert_eq!(cursor["active_profile"], "work");

    // ---- delete refusal while the cursor link points into 'work' ----
    let out = profile_cmd(home, &["delete", "work"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "deleting the active profile must exit CONFIG_ERROR"
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("active"));
    assert!(work_cursor.is_dir(), "refused delete must not remove data");

    // ---- switch back, then delete succeeds ----
    let out = profile_cmd(home, &["use", "default", "--global"]);
    assert_success(&out, "use default --global");
    assert!(cursor_resolves_to(home, &default_cursor));
    let out = profile_cmd(home, &["delete", "work"]);
    assert_success(&out, "delete work after switching away");
    assert!(!store.join("work").exists());

    // ---- unmanaged-dir refusal: real dir at the cursor path ----
    remove_link(&cursor_live);
    std::fs::create_dir_all(&cursor_live).unwrap();
    std::fs::write(cursor_live.join("reinstalled.txt"), "fresh").unwrap();
    let out = profile_cmd(home, &["use", "default", "--global"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "switching over a real directory must be refused"
    );
    assert!(
        cursor_live.join("reinstalled.txt").exists(),
        "the unmanaged directory must be left untouched"
    );
}
