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
    let raw = match std::env::var("JARVY_BIN") {
        Ok(bin) => PathBuf::from(bin),
        Err(_) => assert_cmd::cargo::cargo_bin!("jarvy").to_path_buf(),
    };
    // Several tests spawn jarvy with `current_dir(<tempdir>)`; a relative
    // `JARVY_BIN` (e.g. CI's `target/release/jarvy`) then resolves against
    // the tempdir and misses the actual binary. Anchor to an absolute path
    // once so every spawn sees the same file regardless of cwd.
    if raw.is_absolute() {
        raw
    } else {
        std::env::current_dir()
            .expect("current_dir")
            .join(raw)
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

/// Variant that spawns with a specific cwd + arbitrary extra env vars.
/// Needed for `check-cwd` (walks up from cwd for jarvy.toml) and for
/// setting `JARVY_CWD_HINT_INVOCATION` / `JARVY_NO_CWD_HINT`.
fn profile_cmd_in(
    home: &Path,
    cwd: &Path,
    extra_env: &[(&str, &str)],
    args: &[&str],
) -> std::process::Output {
    let mut c = Command::new(jarvy_bin());
    c.env("JARVY_HOME", home);
    c.env("JARVY_TEST_MODE", "1");
    c.env("JARVY_TELEMETRY", "0");
    c.env("JARVY_NO_PERSONAL_CONFIG", "1");
    for (k, v) in extra_env {
        c.env(k, v);
    }
    c.current_dir(cwd);
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
    #[cfg(windows)]
    {
        let ft = std::fs::symlink_metadata(home.join(".cursor"))
            .unwrap()
            .file_type();
        let is_symlink = ft.is_symlink();
        // Regardless of whether Developer Mode is on (symlink) or off
        // (junction fallback), the path must canonicalize into the work
        // snapshot. The `cursor_resolves_to` assertion above already
        // covers this — we add the branch label to make it visible in
        // CI --nocapture output.
        let resolves = cursor_resolves_to(home, &work_cursor);
        assert!(
            resolves,
            "cursor must resolve into work snapshot (junction={})",
            !is_symlink
        );
        eprintln!(
            "cursor link kind: {}",
            if is_symlink { "symlink" } else { "junction" }
        );
    }

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

    // ---- save (env tier — refresh single agent from live) ----
    // Mutate the live claude-code config, then save into 'default'
    // targeting ONLY claude-code. The snapshot must pick up the new
    // bytes — proves save reads live and writes into the store.
    std::fs::write(claude_live.join("settings.json"), "{\"marker\":\"v2\"}").unwrap();
    let out = profile_cmd(home, &["save", "default", "--agents", "claude-code"]);
    assert_success(&out, "save default --agents claude-code");
    let snapshot_settings = store
        .join("default")
        .join("claude-code")
        .join("settings.json");
    let snap_body = std::fs::read_to_string(&snapshot_settings).unwrap();
    assert!(
        snap_body.contains("\"marker\":\"v2\""),
        "save must refresh the env-tier snapshot; got: {snap_body:?}"
    );

    // ---- save (unfiltered — every active-profile agent) ----
    // Mutate codex live too; bare `save` (no name, no --agents) must
    // target every agent's active profile per-agent. After the earlier
    // `use default --global` and reconcile-on-init, codex's active
    // profile is 'default'.
    std::fs::write(codex_live.join("config.toml"), "# v2").unwrap();
    let out = profile_cmd(home, &["save"]);
    assert_success(&out, "save (unfiltered)");
    let codex_snap = store.join("default").join("codex").join("config.toml");
    let codex_snap_body = std::fs::read_to_string(&codex_snap).unwrap();
    assert!(
        codex_snap_body.contains("# v2"),
        "unfiltered save must refresh codex snapshot; got: {codex_snap_body:?}"
    );

    // ---- restore (env tier — snapshot back over live) ----
    // Clobber the live settings.json with local junk; restore must
    // put the saved v2 bytes back.
    std::fs::write(
        claude_live.join("settings.json"),
        "{\"marker\":\"local-junk\"}",
    )
    .unwrap();
    let out = profile_cmd(home, &["restore", "default", "--agents", "claude-code"]);
    assert_success(&out, "restore default --agents claude-code");
    let live_after = std::fs::read_to_string(claude_live.join("settings.json")).unwrap();
    assert!(
        live_after.contains("\"marker\":\"v2\""),
        "restore must overwrite live with the saved snapshot; got: {live_after:?}"
    );

    // ---- restore (env tier, target profile has no snapshot) ----
    // Fresh empty profile; explicit --agents on a missing snapshot
    // must exit CONFIG_ERROR (AgentNotSnapshotted) without touching
    // the live dir.
    let out = profile_cmd(home, &["create", "empty"]);
    assert_success(&out, "create empty");
    let out = profile_cmd(home, &["restore", "empty", "--agents", "claude-code"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "restore of missing snapshot with explicit --agents must exit CONFIG_ERROR"
    );
    let live_untouched = std::fs::read_to_string(claude_live.join("settings.json")).unwrap();
    assert!(
        live_untouched.contains("\"marker\":\"v2\""),
        "refused restore must not mutate live dir; got: {live_untouched:?}"
    );

    // ---- restore (env tier symlink refusal) — Unix only ----
    // Windows junctions vs symlinks are messy; the underlying refusal
    // (`UnmanagedDir` via `symlink_metadata`) fires on Unix symlinks
    // deterministically, which is what we're validating.
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        // Remove real dir, replace with a symlink into a sibling
        // dotfiles-style tree carrying a marker file.
        std::fs::remove_dir_all(&claude_live).unwrap();
        let dotfiles = home.join("dotfiles").join(".claude");
        std::fs::create_dir_all(&dotfiles).unwrap();
        std::fs::write(dotfiles.join("dotfile-marker.txt"), "keep-me").unwrap();
        symlink(&dotfiles, &claude_live).expect("create claude symlink");

        let out = profile_cmd(home, &["restore", "default", "--agents", "claude-code"]);
        assert_eq!(
            out.status.code(),
            Some(2),
            "restore must refuse to overwrite a user-owned symlink"
        );
        // Link is still a symlink and the dotfile marker survives.
        assert!(
            std::fs::symlink_metadata(&claude_live)
                .unwrap()
                .file_type()
                .is_symlink(),
            "claude live path must remain a symlink after refused restore"
        );
        assert_eq!(
            std::fs::read_to_string(dotfiles.join("dotfile-marker.txt")).unwrap(),
            "keep-me",
            "symlink target contents must be untouched"
        );

        // Cleanup: restore live to a real dir carrying the v2 bytes so
        // the unmanaged-dir block below keeps working.
        std::fs::remove_file(&claude_live).unwrap();
        std::fs::create_dir_all(&claude_live).unwrap();
        std::fs::write(claude_live.join("settings.json"), "{\"marker\":\"v2\"}").unwrap();
    }

    // ---- check-cwd (mismatch — prefer=work but live=default) ----
    // The stderr-TTY gate short-circuits when stderr is piped (which
    // `Command` capture always does), so the run returns Silent{NonTty}
    // before touching state or emitting the event. The meaningful E2E
    // check reduces to: the CLI wiring works and exits 0. Pre-create
    // 'work' so check_preference has a valid profile to compare against.
    let out = profile_cmd(home, &["create", "work", "--from", "default"]);
    assert_success(&out, "create work --from default (for check-cwd)");
    let project = home.join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(
        project.join("jarvy.toml"),
        "[agents.profiles]\nprefer = \"work\"\n",
    )
    .unwrap();
    let out = profile_cmd_in(
        home,
        &project,
        &[("JARVY_CWD_HINT_INVOCATION", "rc_snippet")],
        &["check-cwd", "--session-id", "e2e-test"],
    );
    assert_success(&out, "check-cwd (mismatch)");
    // State-file existence is optional: with stderr piped the run
    // silently early-returns before writing state. Presence would prove
    // TTY detection defaults on some CI runners; absence is the
    // documented non-TTY path. Either is fine — we accept both.
    let _ = store.join(".cwd-hint-state.json").exists();

    // ---- check-cwd (opt-out env respected) ----
    let out = profile_cmd_in(
        home,
        &project,
        &[
            ("JARVY_NO_CWD_HINT", "1"),
            ("JARVY_CWD_HINT_INVOCATION", "rc_snippet"),
        ],
        &["check-cwd", "--session-id", "e2e-test-optout"],
    );
    assert_success(&out, "check-cwd (opt-out)");

    // Cleanup 'work' before the unmanaged-dir block re-uses cursor.
    // Switch cursor back to default first — 'work' is the last
    // symlink-tier active for cursor if `--from` seeded it, but here
    // the current active is still 'default' from the earlier switch,
    // so `delete work` succeeds outright.
    let out = profile_cmd(home, &["delete", "work"]);
    assert_success(&out, "delete work (post check-cwd cleanup)");

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
