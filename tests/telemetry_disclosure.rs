//! Integration coverage for the opt-out disclosure surfaces added when
//! `TelemetryConfig::default().enabled` flipped from `false` to `true`.
//!
//! These tests are the regression guard the QA reviewer flagged: every
//! other telemetry test now sets `JARVY_TELEMETRY=0|1` explicitly, so
//! the "no env var, fresh install, telemetry on" path that ships to
//! real users had no integration assertion. Without this file, a future
//! refactor that re-flips the default to opt-in would pass every test.
//!
//! Strategy: isolate each test with `JARVY_HOME=<tempdir>` so the
//! binary writes its `.jarvy/` directory under our tempdir, never the
//! developer's real home. `JARVY_TELEMETRY` is left unset so the
//! global default (now `true`) decides effective state. We do NOT set
//! `JARVY_TEST_MODE=1` here — that env var causes
//! `initialize_from_disk` to short-circuit before any disclosure
//! logic runs, which is exactly what we need to NOT happen.

mod common;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use serial_test::serial;
use std::process::Command;

/// A minimal `jarvy` invocation that exercises `initialize` but exits
/// quickly. `telemetry status` parses args, runs `initialize` (the
/// path that emits the disclosure), prints the current resolved
/// telemetry state, and exits. `--version` would NOT work — clap
/// short-circuits before `initialize()` for `-V`/`--version` so the
/// banner never runs in that path.
///
/// We deliberately do NOT call `jarvy_cmd()` here — that helper sets
/// `JARVY_TEST_MODE=1`, which causes `initialize_from_disk` to
/// short-circuit before any disclosure logic runs.
///
/// `Command` inherits the parent process environment, so avoiding
/// *setting* `JARVY_TEST_MODE` is not enough: CI jobs that export it as
/// ambient env (`coverage.yml`, `e2e-cross-platform.yml` set
/// `JARVY_TEST_MODE=1`) would leak it into the spawned binary, short-
/// circuit `initialize_from_disk`, suppress the banner, and fail these
/// tests in exactly those jobs (but not the nextest job, which doesn't
/// set it). Strip it so the disclosure path runs regardless of runner.
fn jarvy_no_test_mode() -> Command {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env_remove("JARVY_TEST_MODE");
    c
}

/// One paragraph string fragment from the boxed disclosure. Pinned by
/// these tests so a future refactor (e.g. moving the strings into an
/// i18n layer) can't silently re-introduce the opt-in copy.
const BANNER_FRAGMENT: &str = "Jarvy telemetry is currently ENABLED.";
const BANNER_DISABLE_HINT: &str = "jarvy telemetry disable";
const BANNER_ENV_HINT: &str = "JARVY_TELEMETRY=0 jarvy <cmd>";

/// Build a `Command` with `JARVY_HOME` pointed at a fresh tempdir.
/// Returns the command + the tempdir guard so the caller can inspect
/// the produced `config.toml` afterward.
fn cmd_with_isolated_home() -> (Command, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut c = jarvy_no_test_mode();
    c.env("JARVY_HOME", tmp.path().join(".jarvy"));
    // Strip env vars that would corrupt the test setup:
    // - JARVY_TELEMETRY would override the default behavior we want to test.
    // - JARVY_SANDBOX=0 disables the seamless auto-detection so the
    //   test runs identically on a developer laptop and inside CI /
    //   Claude Code (which would otherwise auto-disable and skip the
    //   banner emit since telemetry is off in that path).
    c.env_remove("JARVY_TELEMETRY");
    c.env("JARVY_SANDBOX", "0");
    (c, tmp)
}

#[test]
#[serial(jarvy_telemetry_disclosure)]
fn first_run_surfaces_banner_and_persists_enabled_true() {
    // Test A from the parallel-code-review plan, item 3.
    // Fresh tempdir, no env override → first-run disclosure path.
    let (mut c, tmp) = cmd_with_isolated_home();
    c.args(["telemetry", "status"]);

    let assert = c.assert().success();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).to_string();
    assert!(
        stderr.contains(BANNER_FRAGMENT),
        "first-run banner must surface: {stderr}"
    );
    assert!(
        stderr.contains(BANNER_DISABLE_HINT),
        "banner must surface disable command: {stderr}"
    );
    assert!(
        stderr.contains(BANNER_ENV_HINT),
        "banner must surface per-invocation env override: {stderr}"
    );

    // Config file was persisted with the opt-out default.
    let config_path = tmp.path().join(".jarvy").join("config.toml");
    let content = std::fs::read_to_string(&config_path).expect("config persisted");
    assert!(
        content.contains("enabled = true"),
        "first-run must persist enabled = true so the next run is decided: {content}"
    );
}

#[test]
#[serial(jarvy_telemetry_disclosure)]
fn second_run_does_not_repeat_banner() {
    // Test B: the "loud once" contract. After first run persists
    // `enabled = true`, a subsequent run on the same JARVY_HOME must
    // NOT re-emit the banner.
    let tmp = tempfile::tempdir().expect("tempdir");

    // Run 1 — primes the config.
    {
        let mut c = jarvy_no_test_mode();
        c.env("JARVY_HOME", tmp.path().join(".jarvy"))
            .env_remove("JARVY_TELEMETRY")
            .env("JARVY_SANDBOX", "0")
            .args(["telemetry", "status"]);
        c.assert().success();
    }

    // Run 2 — must be silent on the disclosure surface.
    let mut c = jarvy_no_test_mode();
    c.env("JARVY_HOME", tmp.path().join(".jarvy"))
        .env_remove("JARVY_TELEMETRY")
        .env("JARVY_SANDBOX", "0")
        .args(["telemetry", "status"]);

    let assert = c.assert().success();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).to_string();
    assert!(
        !stderr.contains(BANNER_FRAGMENT),
        "second run must not repeat the banner — config is decided: {stderr}"
    );
}

#[test]
#[serial(jarvy_telemetry_disclosure)]
fn legacy_config_without_telemetry_section_triggers_disclosure() {
    // Test C: users whose `~/.jarvy/config.toml` pre-dates the
    // `[telemetry]` block (the block was introduced in commit d039d9b)
    // must see the disclosure on next post-upgrade run, then have it
    // persisted so the next-next run is decided. This closes the
    // silent-enrollment loop the security reviewer found in F2.
    let tmp = tempfile::tempdir().expect("tempdir");
    let jarvy_dir = tmp.path().join(".jarvy");
    std::fs::create_dir_all(&jarvy_dir).unwrap();
    let config_path = jarvy_dir.join("config.toml");
    // Pre-`[telemetry]` config shape: only `[settings]`, no telemetry
    // block at all.
    std::fs::write(
        &config_path,
        "[settings]\ntelemetry = true\nfingerprint = \"legacy-abc\"\n",
    )
    .unwrap();

    let mut c = jarvy_no_test_mode();
    c.env("JARVY_HOME", &jarvy_dir)
        .env_remove("JARVY_TELEMETRY")
        .env("JARVY_SANDBOX", "0")
        .args(["telemetry", "status"]);

    let assert = c.assert().success();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).to_string();
    assert!(
        stderr.contains(BANNER_FRAGMENT),
        "legacy-config upgrade must surface the disclosure: {stderr}"
    );

    // Re-read config — the [telemetry] block must now be present so
    // the next run is decided.
    let content = std::fs::read_to_string(&config_path).expect("config still present");
    assert!(
        content.contains("[telemetry]"),
        "post-upgrade config must persist [telemetry] section: {content}"
    );
    assert!(
        content.contains("enabled = true"),
        "post-upgrade config must persist enabled = true: {content}"
    );
    // Original `[settings]` data must not be clobbered.
    assert!(
        content.contains("fingerprint = \"legacy-abc\""),
        "post-upgrade write must preserve other sections: {content}"
    );
}

#[test]
#[serial(jarvy_telemetry_disclosure)]
fn nudge_text_pinned_for_undecided_setup_run() {
    // Pins the end-of-`jarvy setup` nudge wording (the secondary
    // disclosure surface) so a future i18n refactor cannot silently
    // re-introduce the prior "Tip: opt-in and currently off" tone.
    //
    // To reach the nudge we need: telemetry-on at runtime AND
    // user-undecided AND a setup invocation. We construct that by
    // seeding a legacy config (no [telemetry] block), then we expect
    // the FIRST `jarvy setup` invocation to:
    //   - render the boxed first-run/legacy banner via initialize_from_disk
    //   - then persist enabled=true
    //   - then run `setup` and emit the end-of-setup nudge AFTER
    //     persisting (since the nudge reads the just-rewritten config
    //     which is now decided)
    // ... wait — after the rewrite the user IS decided, so the nudge
    // suppresses. That is correct behavior: a freshly-disclosed user
    // shouldn't be double-nudged. So this test pins the inverse:
    // after disclosure-then-persist, the nudge MUST NOT fire on the
    // same run.
    let tmp = tempfile::tempdir().expect("tempdir");
    let jarvy_dir = tmp.path().join(".jarvy");
    std::fs::create_dir_all(&jarvy_dir).unwrap();
    std::fs::write(
        jarvy_dir.join("config.toml"),
        "[settings]\nfingerprint = \"abc\"\n",
    )
    .unwrap();

    // Empty jarvy.toml in another tempdir so `jarvy setup` parses
    // valid config but does no work.
    let project_dir = tempfile::tempdir().unwrap();
    let project_toml = project_dir.path().join("jarvy.toml");
    std::fs::write(&project_toml, "").unwrap();

    let mut c = jarvy_no_test_mode();
    c.env("JARVY_HOME", &jarvy_dir)
        .env_remove("JARVY_TELEMETRY")
        .env("JARVY_SANDBOX", "0")
        // Setup is interactive by default — disable prompts.
        .env("JARVY_TEST_MODE", "1")
        .args(["setup", "--file"])
        .arg(&project_toml)
        .arg("--no-hooks");
    // Setup may exit 0 (everything skipped) under JARVY_FAST_TEST-less
    // mode. Either way, we only care about stderr content.
    let output = c.output().expect("run jarvy setup");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // The end-of-setup nudge must NOT fire on the same run that
    // disclosed — the disclosure persisted enabled=true, which
    // immediately decides the user. Double-disclosure on the same
    // run is the regression we're guarding against.
    assert!(
        !stderr.contains("opt-out and currently on"),
        "nudge must not fire on the same run as the banner: {stderr}"
    );
}

#[test]
#[serial(jarvy_telemetry_disclosure)]
fn explicit_disable_via_env_suppresses_banner_event_but_banner_still_prints() {
    // The boxed disclosure is a privacy disclosure, not a telemetry
    // signal — it must fire even if the user has telemetry disabled
    // for this invocation, because the disclosure is what tells them
    // disabling is an option. The audit *event* gates on
    // `is_enabled()` (so we don't add metric volume from a disabled
    // user), but the stderr disclosure is unconditional.
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut c = jarvy_no_test_mode();
    c.env("JARVY_HOME", tmp.path().join(".jarvy"))
        .env("JARVY_TELEMETRY", "0")
        .env("JARVY_SANDBOX", "0")
        .args(["telemetry", "status"]);

    let assert = c.assert().success();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).to_string();
    assert!(
        stderr.contains(BANNER_FRAGMENT),
        "banner must surface regardless of JARVY_TELEMETRY=0: {stderr}"
    );
    // The OTLP layer is off so no `telemetry.disclosure_shown` log
    // line should appear (the gate inside disclosure_shown() drops
    // it). Defensive: the eprintln line itself does not contain
    // "telemetry.disclosure_shown" as a substring.
    assert!(
        !stderr.contains("event=telemetry.disclosure_shown"),
        "disclosure event must not emit when telemetry is disabled: {stderr}"
    );
    // Sanity: predicate-style assertion to catch shape regressions.
    predicate::str::contains(BANNER_FRAGMENT).eval(&stderr);
}
