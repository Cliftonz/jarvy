//! Integration tests for the observability CLI surface (PRD-027 T16).
//!
//! Exercises the flags end-to-end through the real binary:
//! `doctor --check` category filtering + `--format json`, `doctor
//! --extended`, `setup --profile` / `--profile-output`, `setup
//! --log-format json`, and `diagnose <tool>` / `diagnose --export`.
//!
//! Everything runs in dry-run / test mode with `HOME` redirected to a
//! tempdir, so no host state is touched.

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::io::Write;
use std::process::Command;
use tempfile::{NamedTempFile, TempDir};

/// A jarvy binary invocation with the noise sources (interactive prompts,
/// seamless-mode banner, telemetry) disabled and `HOME` isolated.
fn jarvy(home: &TempDir) -> Command {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1")
        .env("JARVY_SANDBOX", "0")
        .env("JARVY_TELEMETRY", "0")
        .env("JARVY_FAST_TEST", "1")
        .env("HOME", home.path());
    c
}

fn minimal_config() -> NamedTempFile {
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

// ===== doctor --check category filtering =====

#[test]
fn doctor_check_tools_shows_only_tool_health() {
    let home = TempDir::new().unwrap();
    let out = jarvy(&home)
        .args(["doctor", "--check", "tools", "--tools", "git"])
        .assert()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    // System info is always the context header; Tool Health is selected;
    // PATH Analysis and Hooks Status must be absent.
    assert!(s.contains("System Information"), "system header expected");
    assert!(s.contains("Tool Health"), "tool section expected");
    assert!(
        !s.contains("PATH Analysis"),
        "PATH section must be filtered out:\n{s}"
    );
    assert!(
        !s.contains("Hooks Status"),
        "Hooks section must be filtered out:\n{s}"
    );
}

#[test]
fn doctor_check_path_hooks_excludes_tools() {
    let home = TempDir::new().unwrap();
    let out = jarvy(&home)
        .args(["doctor", "--check", "path,hooks"])
        .assert()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("PATH Analysis"), "PATH section expected");
    assert!(
        !s.contains("Tool Health"),
        "Tool section must be filtered out:\n{s}"
    );
}

#[test]
fn doctor_check_unknown_category_errors() {
    let home = TempDir::new().unwrap();
    jarvy(&home)
        .args(["doctor", "--check", "bogus"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("unknown doctor category"));
}

// ===== doctor output formats =====

#[test]
fn doctor_format_json_is_valid_json() {
    let home = TempDir::new().unwrap();
    let out = jarvy(&home)
        .args(["doctor", "--tools", "git", "--format", "json"])
        .assert()
        .get_output()
        .stdout
        .clone();
    let parsed: serde_json::Value =
        serde_json::from_slice(&out).expect("doctor --format json must emit valid JSON");
    assert!(
        parsed.get("system").is_some() && parsed.get("tools").is_some(),
        "doctor JSON should carry system + tools keys"
    );
}

#[test]
fn doctor_extended_runs() {
    let home = TempDir::new().unwrap();
    let out = jarvy(&home)
        .args(["doctor", "--extended", "--tools", "git"])
        .assert()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("System Metrics") || s.contains("Tool Summary"));
}

// ===== setup --profile =====

#[test]
fn setup_profile_emits_report_to_stderr() {
    let home = TempDir::new().unwrap();
    let cfg = minimal_config();
    let out = jarvy(&home)
        .args(["setup", "--dry-run", "--profile", "--file"])
        .arg(cfg.path())
        .assert()
        .get_output()
        .stderr
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(
        s.contains("Performance Profile"),
        "profile summary expected on stderr:\n{s}"
    );
    assert!(
        s.contains("version_check"),
        "at least one phase name expected:\n{s}"
    );
}

#[test]
fn setup_profile_output_writes_ms_json() {
    let home = TempDir::new().unwrap();
    let cfg = minimal_config();
    let profile_out = home.path().join("profile.json");
    jarvy(&home)
        .args(["setup", "--dry-run", "--profile", "--profile-output"])
        .arg(&profile_out)
        .args(["--file"])
        .arg(cfg.path())
        .assert()
        .success();
    let body = std::fs::read_to_string(&profile_out).expect("profile-output file written");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("profile JSON valid");
    assert!(
        parsed.get("total_duration_ms").is_some(),
        "durations must serialize as integer *_ms, got: {body}"
    );
    assert!(!body.contains("nanos"), "no raw Duration encoding: {body}");
}

#[test]
fn setup_log_format_json_still_succeeds() {
    let home = TempDir::new().unwrap();
    let cfg = minimal_config();
    // --log-format json switches the console layer to JSON; the command
    // must still complete (the flag previously parsed but did nothing).
    jarvy(&home)
        .args(["setup", "--dry-run", "--log-format", "json", "--file"])
        .arg(cfg.path())
        .assert()
        .success();
}

// ===== diagnose =====

#[test]
fn diagnose_known_tool_runs() {
    let home = TempDir::new().unwrap();
    let out = jarvy(&home)
        .args(["diagnose", "git"])
        .assert()
        .get_output()
        .stdout
        .clone();
    assert!(String::from_utf8_lossy(&out).contains("Diagnosing: git"));
}

/// Startup one-shots (`shell-init` / `ensure` / `completions`) must
/// leave stderr empty on the common path. INFO tracing or the seamless
/// banner leaking to stderr on every new terminal was the user
/// complaint that motivated the WarnOnly console cap.
///
/// The file appender at `~/.jarvy/logs/jarvy.log` still writes at INFO;
/// this test only pins the *console* silence.
#[test]
fn shell_init_stderr_is_silent() {
    let home = TempDir::new().unwrap();
    let mut c = jarvy(&home);
    // Force a sandbox provider so the banner code path is exercised
    // (if it wasn't muted, stderr would carry the banner line).
    c.env("CLAUDECODE", "1");
    c.env("JARVY_HOME", home.path());
    c.args(["shell-init", "--shell", "zsh"]);
    let output = c.assert().success().get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.is_empty(),
        "shell-init leaked to stderr: {stderr:?}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("jarvy ensure"),
        "shell-init snippet missing from stdout: {stdout:?}"
    );
}

/// `ensure --quiet` (the rc-snippet invocation shape) must be silent
/// even under a forced seamless-mode sandbox. Renamed from the earlier
/// `..._in_seamless_mode` to reflect that `--quiet` (not the sandbox)
/// is what forces the silence path — the test name previously misled.
#[test]
fn ensure_stderr_silent_with_quiet_flag() {
    let home = TempDir::new().unwrap();
    let mut c = jarvy(&home);
    c.env("CLAUDECODE", "1");
    c.env("JARVY_HOME", home.path());
    c.args(["ensure", "--quiet"]);
    let output = c.assert().success().get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.is_empty(), "ensure leaked to stderr: {stderr:?}");
}

/// **WARN reaches stderr under the WarnOnly cap.**
///
/// The whole point of `LogLevel::WarnOnly` is that actionable warnings
/// still surface. Without this guard, a one-line change of the cap to
/// `LevelFilter::ERROR` would silently regress every operator warning.
///
/// Seeds `$JARVY_HOME/tools.d` with a world-writable dir on Unix —
/// triggers the `plugins.tools_d_unsafe_perms` WARN inside
/// `tools::register_all()` (runs on every jarvy invocation). Asserts
/// WARN reaches stderr but INFO (e.g. `plugins.registered`) does not.
#[cfg(unix)]
#[test]
fn shell_init_warn_reaches_stderr_under_warnonly() {
    use std::os::unix::fs::PermissionsExt;
    let home = TempDir::new().unwrap();
    let jarvy_home = home.path().join(".jarvy");
    std::fs::create_dir_all(jarvy_home.join("tools.d")).unwrap();
    std::fs::set_permissions(
        jarvy_home.join("tools.d"),
        std::fs::Permissions::from_mode(0o777),
    )
    .unwrap();

    let mut c = jarvy(&home);
    c.env("JARVY_HOME", &jarvy_home);
    c.args(["shell-init", "--shell", "zsh"]);
    let output = c.assert().success().get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("tools_d_unsafe_perms") || stderr.contains("insecure permissions"),
        "WARN must survive WarnOnly cap and reach stderr: {stderr:?}"
    );
    assert!(
        !stderr.contains("plugins.registered"),
        "INFO must be suppressed under WarnOnly: {stderr:?}"
    );
}

/// `shell-init -v` restores INFO on the console — escape hatch for
/// debugging a broken rc snippet. Asserts on the jarvy-owned witness
/// event (`shell_init.started`) rather than an unrelated event —
/// decouples the test from refactors of plugin loading.
#[test]
fn shell_init_verbose_reopens_info_on_console() {
    let home = TempDir::new().unwrap();
    let mut c = jarvy(&home);
    c.env("JARVY_HOME", home.path());
    c.args(["shell-init", "--shell", "zsh", "-v"]);
    let output = c.assert().success().get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("shell_init.started"),
        "-v must restore INFO tracing on stderr: {stderr:?}"
    );
}

/// The rc snippet emitted by `shell-init` must include the log-file
/// lead (`|| echo` or per-shell equivalent) so a broken `ensure` no
/// longer loops invisibly on shell startup. Also verifies the
/// `JARVY_ENSURE_INVOCATION=rc_snippet` marker for telemetry
/// attribution of rc-triggered runs.
#[test]
fn shell_init_snippet_carries_failure_surface_and_invocation_marker() {
    for shell in ["bash", "zsh", "sh", "fish", "powershell", "nushell"] {
        let home = TempDir::new().unwrap();
        let mut c = jarvy(&home);
        c.args(["shell-init", "--shell", shell]);
        let output = c.assert().success().get_output().clone();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("~/.jarvy/logs/jarvy.log"),
            "{shell} snippet missing log-file lead: {stdout:?}"
        );
        assert!(
            stdout.contains("JARVY_ENSURE_INVOCATION"),
            "{shell} snippet missing rc-invocation marker: {stdout:?}"
        );
    }
}

#[test]
fn diagnose_export_writes_json_report() {
    let home = TempDir::new().unwrap();
    // --export writes jarvy-diagnose-<tool>-<ts>.json into the cwd.
    let workdir = TempDir::new().unwrap();
    jarvy(&home)
        .current_dir(workdir.path())
        .args(["diagnose", "git", "--export"])
        .assert()
        .success();
    let written: Vec<_> = std::fs::read_dir(workdir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| {
            let n = e.file_name();
            let n = n.to_string_lossy();
            n.starts_with("jarvy-diagnose-git-") && n.ends_with(".json")
        })
        .collect();
    assert_eq!(
        written.len(),
        1,
        "exactly one diagnose export file expected"
    );
    let body = std::fs::read_to_string(written[0].path()).unwrap();
    serde_json::from_str::<serde_json::Value>(&body).expect("diagnose export is valid JSON");
}
