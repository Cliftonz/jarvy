//! End-to-end tests for the console chatter gate.
//!
//! Pin the four cells of the precedence table (see `src/console.rs`):
//!   1. Non-TTY default → silent (both stdout narration AND stderr
//!      `INFO ... event="..."` tracing suppressed).
//!   2. `[logging] chatter = true` in `jarvy.toml` → both reappear.
//!   3. `JARVY_CHATTER=1` env → both reappear (beats toml `false`).
//!   4. `-v` on setup → both reappear (non-TTY doesn't defeat it).
//!
//! `assert_cmd` always spawns the child with piped stdout/stderr, so
//! `stderr.is_terminal()` is `false` inside the child — same code path
//! as an npm predev / CI invocation.

use std::io::Write;
use std::process::Command;
use tempfile::{NamedTempFile, TempDir};

/// Chatter sentinel that lives inside `setup_cmd::run_setup` at an
/// unconditional call site — always fires when chatter is on, never
/// when off. Free of DRY-RUN gating so the test can drop the OS-level
/// installer path via `--dry-run` and still assert.
const CHATTER_SENTINEL: &str = "Checking tool versions...";

/// Tracing sentinel — a fragment of the first INFO event `jarvy setup`
/// emits via the console layer. Present when console tracing runs at
/// INFO (chatter on), absent when the WarnOnly cap kicks in (off).
///
/// Deliberately does NOT include `event=` — the tracing_subscriber
/// text formatter interleaves ANSI colour codes between the field name
/// and the `=`, so `event="setup.start"` never appears as a contiguous
/// substring. The value literal itself is uncolored and safe to match.
const TRACING_SENTINEL: &str = "\"setup.start\"";

fn jarvy(home: &TempDir) -> Command {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1")
        .env("JARVY_SANDBOX", "0")
        .env("JARVY_TELEMETRY", "0")
        .env("JARVY_FAST_TEST", "1")
        .env("JARVY_MCP_REGISTER", "0")
        .env("JARVY_NO_PERSONAL_CONFIG", "1")
        .env("HOME", home.path())
        .env("JARVY_HOME", home.path())
        .env_remove("JARVY_CHATTER")
        .env_remove("CLAUDECODE");
    c
}

fn write_config(chatter: Option<bool>) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    let logging = match chatter {
        Some(v) => format!("\n[logging]\nchatter = {v}\n"),
        None => String::new(),
    };
    writeln!(
        f,
        r#"[privileges]
use_sudo = false

[provisioner]
git = "1.0.0"
{logging}"#
    )
    .unwrap();
    f
}

fn run_setup(
    mut cmd: Command,
    cfg_path: &std::path::Path,
    extra_args: &[&str],
) -> (String, String) {
    // Deliberately NOT `--dry-run` — dry-run folds into the `verbose`
    // axis in `console::init` (the whole point of dry-run is to *show*
    // the plan), so a chatter test that used `--dry-run` for speed
    // would tautologically pass. `JARVY_FAST_TEST=1` short-circuits
    // external command execution instead, keeping the run fast without
    // biasing the chatter gate.
    cmd.args(["setup", "--file"]).arg(cfg_path).args(extra_args);
    let output = cmd.output().expect("failed to spawn jarvy");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn non_tty_default_suppresses_chatter_and_info_tracing() {
    let home = TempDir::new().unwrap();
    let cfg = write_config(None);
    let (stdout, stderr) = run_setup(jarvy(&home), cfg.path(), &[]);

    assert!(
        !stdout.contains(CHATTER_SENTINEL),
        "chatter should be off under non-TTY default; got stdout:\n{stdout}"
    );
    assert!(
        !stderr.contains(TRACING_SENTINEL),
        "INFO tracing should be capped under non-TTY default; got stderr:\n{stderr}"
    );
}

#[test]
fn toml_chatter_true_reopens_narration_and_tracing() {
    let home = TempDir::new().unwrap();
    let cfg = write_config(Some(true));
    let (stdout, stderr) = run_setup(jarvy(&home), cfg.path(), &[]);

    assert!(
        stdout.contains(CHATTER_SENTINEL),
        "[logging] chatter = true should re-enable narration; stdout:\n{stdout}"
    );
    assert!(
        stderr.contains(TRACING_SENTINEL),
        "[logging] chatter = true should restore INFO tracing to stderr; stderr:\n{stderr}"
    );
}

#[test]
fn env_var_beats_toml_false() {
    let home = TempDir::new().unwrap();
    let cfg = write_config(Some(false));
    let mut cmd = jarvy(&home);
    cmd.env("JARVY_CHATTER", "1");
    let (stdout, stderr) = run_setup(cmd, cfg.path(), &[]);

    assert!(
        stdout.contains(CHATTER_SENTINEL),
        "JARVY_CHATTER=1 must beat [logging] chatter = false; stdout:\n{stdout}"
    );
    assert!(
        stderr.contains(TRACING_SENTINEL),
        "JARVY_CHATTER=1 must restore INFO tracing; stderr:\n{stderr}"
    );
}

#[test]
fn env_var_off_beats_verbose_flag() {
    // Precedence pin: `JARVY_CHATTER=0` is the highest-priority signal,
    // so it must silence even an explicit `-v` on the same invocation.
    let home = TempDir::new().unwrap();
    let cfg = write_config(None);
    let mut cmd = jarvy(&home);
    cmd.env("JARVY_CHATTER", "0");
    let (stdout, _) = run_setup(cmd, cfg.path(), &["-v"]);

    assert!(
        !stdout.contains(CHATTER_SENTINEL),
        "JARVY_CHATTER=0 must suppress narration even with -v; stdout:\n{stdout}"
    );
}

#[test]
fn verbose_flag_reopens_when_non_tty() {
    let home = TempDir::new().unwrap();
    let cfg = write_config(None);
    let (stdout, stderr) = run_setup(jarvy(&home), cfg.path(), &["-v"]);

    assert!(
        stdout.contains(CHATTER_SENTINEL),
        "-v must reopen narration even under non-TTY; stdout:\n{stdout}"
    );
    assert!(
        stderr.contains(TRACING_SENTINEL),
        "-v must restore INFO tracing; stderr:\n{stderr}"
    );
}

#[test]
fn quiet_flag_forces_off_even_with_toml_true() {
    let home = TempDir::new().unwrap();
    let cfg = write_config(Some(true));
    let (stdout, _) = run_setup(jarvy(&home), cfg.path(), &["--quiet"]);

    assert!(
        !stdout.contains(CHATTER_SENTINEL),
        "--quiet must suppress narration even with [logging] chatter = true; stdout:\n{stdout}"
    );
}
