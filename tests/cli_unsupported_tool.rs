//! Integration tests for the unsupported-tool feedback loop.
//!
//! Covers the canonical-telemetry-channel flow:
//! 1. `jarvy tools --request <name>` always sends via telemetry (bypasses
//!    opt-in) and reports "Reported via telemetry" instead of pushing
//!    the user toward a GitHub account.
//! 2. `--format json` emits a machine-readable payload carrying
//!    `channel: "telemetry"` so AI agents know the request landed.
//! 3. Requesting an *already supported* tool short-circuits.
//! 4. `jarvy setup` against a config of only unsupported tools exits
//!    with `TOOL_UNSUPPORTED` (8). With telemetry off, the fallback
//!    GitHub URL is surfaced as the only remaining channel.

mod common;

use assert_cmd::prelude::*;
use common::jarvy_cmd;
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

/// Stable exit code from `src/error_codes.rs`. Hardcoded here so the
/// test fails loudly if the code is ever renumbered — the AI contract
/// depends on this number being constant.
const TOOL_UNSUPPORTED: i32 = 8;

#[test]
fn tools_request_pretty_confirms_telemetry_send() {
    // `--request` always sends via telemetry. Human output should
    // confirm the send and avoid pushing the GitHub URL when the
    // canonical channel handled the request.
    let mut c = jarvy_cmd();
    c.args(["tools", "--request", "definitely-not-a-real-tool"]);

    c.assert()
        .success()
        .stdout(predicate::str::contains(
            "tool `definitely-not-a-real-tool` is not in the Jarvy registry",
        ))
        .stdout(predicate::str::contains("Reported via telemetry"))
        .stdout(predicate::str::contains(
            "cargo run -p cargo-jarvy -- new-tool definitely-not-a-real-tool",
        ))
        .stdout(predicate::str::contains("define_tool!"))
        // GitHub URL must not be the primary CTA when telemetry already
        // handled the request — that's the whole point of the channel.
        .stdout(predicate::str::contains("github.com").not());
}

#[test]
fn tools_request_json_is_machine_readable() {
    let mut c = jarvy_cmd();
    c.args([
        "tools",
        "--request",
        "zzz-fake-tool-name",
        "--format",
        "json",
    ]);

    let assert = c.assert().success();
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref()).to_string();

    let v: serde_json::Value =
        serde_json::from_str(&stdout).expect("--request --format json must emit valid JSON");

    assert_eq!(v["kind"], "unsupported_tool");
    assert_eq!(v["tool"], "zzz-fake-tool-name");
    assert_eq!(v["exit_code"], TOOL_UNSUPPORTED);
    // Canonical channel is telemetry — AI agents read this to know the
    // request landed without needing to fire the fallback URL.
    assert_eq!(v["channel"], "telemetry");
    assert!(
        v["fallback_issue_url"]
            .as_str()
            .unwrap()
            .contains("tool_request.yml"),
        "fallback_issue_url should still be populated for users who want a public record"
    );
    assert!(v["snippet"].is_string(), "JSON payload must carry snippet");
    assert!(v["suggestions"].is_array());
}

#[test]
fn tools_request_known_tool_short_circuits() {
    // `git` is a built-in — should refuse to generate a request URL.
    let mut c = jarvy_cmd();
    c.args(["tools", "--request", "git"]);

    c.assert()
        .success()
        .stderr(predicate::str::contains("already supported"));
}

#[test]
fn tools_request_suggests_close_matches() {
    // `gti` is one transposition away from `git`; fuzzy_suggest should
    // surface it in the JSON suggestions list.
    let mut c = jarvy_cmd();
    c.args(["tools", "--request", "gti", "--format", "json"]);

    let assert = c.assert().success();
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref()).to_string();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let suggestions: Vec<String> = v["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect();
    assert!(
        suggestions.contains(&"git".to_string()),
        "suggestions should contain `git`, got: {:?}",
        suggestions
    );
}

#[test]
fn setup_with_only_unsupported_tools_exits_8() {
    // Build a jarvy.toml with a single fictional tool. Setup should:
    // - print the structured unsupported-tool message to stderr
    // - exit with TOOL_UNSUPPORTED (8)
    // With telemetry off (default in tests), the fallback GitHub URL
    // is surfaced because telemetry isn't covering the request.
    let mut cfg = NamedTempFile::new().unwrap();
    writeln!(
        cfg,
        r#"
[provisioner]
totally-fake-tool-xyz = "1.0"
"#
    )
    .unwrap();

    let mut c = jarvy_cmd();
    // JARVY_FAST_TEST skips actual command execution so we don't hit
    // the host's package manager during the test. Telemetry stays off.
    // JARVY_SANDBOX=0 disables sandbox auto-detection so the test runs
    // the same way on Claude Code / containerized CI as on bare metal
    // (otherwise seamless mode flips the renderer into `Sent` and the
    // fallback URL gets suppressed).
    c.env("JARVY_FAST_TEST", "1");
    c.env("JARVY_SANDBOX", "0");
    c.args(["setup", "--file"])
        .arg(cfg.path())
        .arg("--no-hooks");

    c.assert()
        .code(TOOL_UNSUPPORTED)
        .stderr(predicate::str::contains(
            "tool `totally-fake-tool-xyz` is not in the Jarvy registry",
        ))
        // Telemetry is off in this test — fallback URL is the only
        // channel and must be visible.
        .stderr(predicate::str::contains("Telemetry off"))
        .stderr(predicate::str::contains("template=tool_request.yml"))
        .stderr(predicate::str::contains(
            "cargo run -p cargo-jarvy -- new-tool totally-fake-tool-xyz",
        ));
}

#[test]
fn setup_with_unsupported_tool_and_telemetry_on_hides_url() {
    // When telemetry is enabled, setup should report via the canonical
    // channel and NOT surface the GitHub URL — the whole point of the
    // channel rework is reducing friction toward filing issues.
    let mut cfg = NamedTempFile::new().unwrap();
    writeln!(
        cfg,
        r#"
[provisioner]
totally-fake-tool-xyz = "1.0"
"#
    )
    .unwrap();

    let mut c = jarvy_cmd();
    c.env("JARVY_FAST_TEST", "1");
    c.env("JARVY_SANDBOX", "0");
    // Force telemetry on; endpoint doesn't need to resolve for the
    // human-renderer branch — `is_enabled()` just checks the flag.
    c.env("JARVY_TELEMETRY", "1");
    c.args(["setup", "--file"])
        .arg(cfg.path())
        .arg("--no-hooks");

    c.assert()
        .code(TOOL_UNSUPPORTED)
        .stderr(predicate::str::contains("Reporting via telemetry"))
        .stderr(predicate::str::contains("github.com").not());
}
