//! PTY-driven e2e tests for the interactive menu (`jarvy` with no
//! subcommand, `src/interactive.rs`).
//!
//! These spawn the real binary inside a pseudo-terminal via `expectrl`
//! and script the inquire prompts: type-to-filter, Enter to select,
//! Esc to cancel. Unlike `JARVY_TEST_MODE` tests, these exercise the
//! actual TTY code path — raw mode, ANSI rendering, the confirm
//! gauntlet for custom `[commands]` entries.
//!
//! Environment is built from scratch (`env_clear`) so CI/sandbox
//! detection (`sandbox::is_seamless`) can't suppress the flows: all
//! provider checks are env-var based, and the generic-container
//! fallback requires stdin NOT be a TTY — the PTY defeats it.
//!
//! Unix-only: conpty rendering on Windows differs enough that the
//! expect patterns would be flaky there.

#![cfg(unix)]

use expectrl::{Eof, Expect, Session, session::OsSession};
use std::process::Command;
use std::time::Duration;

const EXPECT_TIMEOUT: Duration = Duration::from_secs(30);
/// ESC key in raw mode — inquire maps it to `OperationCanceled`.
const ESC: &str = "\x1b";
/// Enter key in raw mode (PTYs deliver CR, not LF).
const ENTER: &str = "\r";

struct TestEnv {
    /// Project dir (cwd for the spawned jarvy) — holds jarvy.toml.
    project: tempfile::TempDir,
    /// Isolated `JARVY_HOME`.
    home: tempfile::TempDir,
}

impl TestEnv {
    fn new() -> Self {
        Self {
            project: tempfile::tempdir().expect("project tempdir"),
            home: tempfile::tempdir().expect("home tempdir"),
        }
    }

    fn write_jarvy_toml(&self, contents: &str) {
        std::fs::write(self.project.path().join("jarvy.toml"), contents).expect("write jarvy.toml");
    }

    /// Create the first-run marker so the returning-user menu shows.
    fn mark_initialized(&self) {
        std::fs::write(self.home.path().join(".jarvy_initialized"), "").expect("write marker");
    }

    fn spawn(&self) -> OsSession {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_jarvy"));
        cmd.current_dir(self.project.path());
        // Scrubbed env: no CI/sandbox vars leak in, so the interactive
        // flows are deterministic on developer machines AND CI runners.
        cmd.env_clear();
        cmd.env("PATH", std::env::var("PATH").unwrap_or_default());
        cmd.env("HOME", self.home.path());
        cmd.env("TERM", "xterm-256color");
        cmd.env("JARVY_HOME", self.home.path());
        cmd.env("JARVY_TELEMETRY", "0");
        cmd.env("JARVY_NO_PERSONAL_CONFIG", "1");
        cmd.env("JARVY_NO_CWD_HINT", "1");
        cmd.env("JARVY_UPDATE", "0");
        let mut session = Session::spawn(cmd).expect("spawn jarvy in pty");
        session.set_expect_timeout(Some(EXPECT_TIMEOUT));
        session
    }
}

#[test]
fn returning_user_menu_lists_defaults_and_extras_esc_cancels() {
    let env = TestEnv::new();
    env.mark_initialized();
    env.write_jarvy_toml("[commands]\nformat = \"echo formatted-ok\"\n");

    let mut s = env.spawn();
    s.expect("J A R V Y").expect("logo renders");
    s.expect("What would you like to do today?")
        .expect("menu prompt");
    s.expect("Run the project").expect("default option");
    s.expect("Run `format`").expect("[commands] extra surfaced");

    s.send(ESC).expect("send esc");
    s.expect("No choice was made").expect("cancel message");
    s.expect(Eof).expect("process exits");
}

#[test]
fn filter_narrows_and_custom_command_defaults_to_cancelled() {
    let env = TestEnv::new();
    env.mark_initialized();
    env.write_jarvy_toml("[commands]\nformat = \"echo formatted-ok\"\n");

    let mut s = env.spawn();
    s.expect("What would you like to do today?")
        .expect("menu prompt");

    // Type-to-filter (the fzf-style interaction) down to the extra,
    // then select it.
    s.send("format").expect("type filter");
    s.expect("Run `format`").expect("filtered option visible");
    s.send(ENTER).expect("select");

    // Custom (non-default) command → security confirm, default No.
    s.expect("[SECURITY]").expect("confirm banner");
    s.expect("Execute this command?").expect("confirm prompt");
    s.send(ENTER).expect("accept default (No)");
    s.expect("Command cancelled.").expect("declined");
    let tail = s.expect(Eof).expect("process exits");
    // EP-3: the command body must NEVER run when the confirm defaults to
    // No. Without a negative assertion this test would still pass if
    // "Command cancelled." printed AND the command ran anyway.
    let tail_text = String::from_utf8_lossy(tail.before());
    assert!(
        !tail_text.contains("formatted-ok"),
        "declined command body executed anyway; post-cancel output:\n{tail_text}",
    );
    // EP-9: after typing "format" the non-matching default options must
    // be filtered out of the visible frame. Pre-EP-9 the test only
    // verified the matching option was present, so a broken filter
    // (everything shown) would still pass.
    assert!(
        !tail_text.contains("Test the project"),
        "filter did not narrow — non-matching option still visible:\n{tail_text}",
    );
}

#[test]
fn custom_command_confirm_yes_executes() {
    let env = TestEnv::new();
    env.mark_initialized();
    env.write_jarvy_toml("[commands]\nhello = \"echo hello-from-pty\"\n");

    let mut s = env.spawn();
    s.expect("What would you like to do today?")
        .expect("menu prompt");

    s.send("hello").expect("type filter");
    s.expect("Run `hello`").expect("filtered option visible");
    s.send(ENTER).expect("select");

    s.expect("Execute this command?").expect("confirm prompt");
    s.send("y").expect("confirm yes");
    s.send(ENTER).expect("submit");

    s.expect("Running hello command").expect("execution banner");
    s.expect("hello-from-pty")
        .expect("command output reaches pty");
    s.expect(Eof).expect("process exits");
}

#[test]
fn first_run_shows_welcome_and_skip_prints_hints() {
    let env = TestEnv::new();
    // No marker → first-run flow.

    let mut s = env.spawn();
    s.expect("How would you like to get started?")
        .expect("first-run prompt");
    s.expect("Run quickstart (guided setup)")
        .expect("quickstart option");

    s.send("Skip").expect("filter to skip option");
    s.expect("Skip for now").expect("filtered option visible");
    s.send(ENTER).expect("select skip");

    s.expect("You can always run these later")
        .expect("skip hints printed");
    s.expect("jarvy quickstart").expect("hint lists quickstart");
    s.expect(Eof).expect("process exits");
}

#[test]
fn setup_slot_custom_command_confirm_yes_executes() {
    let env = TestEnv::new();
    env.mark_initialized();
    env.write_jarvy_toml("[commands]\nsetup = \"echo setup-ran-ok\"\n");

    let mut s = env.spawn();
    s.expect("What would you like to do today?")
        .expect("menu prompt");

    // "setup" filters down to "Development environment setup".
    s.send("setup").expect("type filter");
    s.expect("Development environment setup")
        .expect("setup option visible");
    s.send(ENTER).expect("select setup");

    // Custom [commands] setup replaces the built-in setup phase and
    // must pass the same confirm gauntlet as any custom command.
    s.expect("[SECURITY]").expect("confirm banner");
    s.expect("Execute this command?").expect("confirm prompt");
    s.send("y").expect("confirm yes");
    s.send(ENTER).expect("submit");

    s.expect("Running setup command").expect("execution banner");
    s.expect("setup-ran-ok")
        .expect("command output reaches pty");
    s.expect(Eof).expect("process exits");
}

#[test]
fn setup_slot_custom_command_explicit_no_cancels() {
    let env = TestEnv::new();
    env.mark_initialized();
    env.write_jarvy_toml("[commands]\nsetup = \"echo should-not-run\"\n");

    let mut s = env.spawn();
    s.expect("What would you like to do today?")
        .expect("menu prompt");

    s.send("setup").expect("type filter");
    s.send(ENTER).expect("select setup");

    s.expect("Execute this command?").expect("confirm prompt");
    // Explicit "n" (not just the default-No Enter path).
    s.send("n").expect("decline");
    s.send(ENTER).expect("submit");

    s.expect("Command cancelled.").expect("declined");
    let tail = s.expect(Eof).expect("process exits");
    // EP-3: explicit "n" must not run the command body. Without this
    // assertion a bug that prints "Command cancelled." AND executes
    // would still slip through.
    let tail_text = String::from_utf8_lossy(tail.before());
    assert!(
        !tail_text.contains("should-not-run"),
        "declined command body executed anyway; post-cancel output:\n{tail_text}",
    );
}

#[test]
fn chained_command_is_refused_without_any_prompt() {
    let env = TestEnv::new();
    env.mark_initialized();
    // `&&` carries `&` — a HARD_BLOCKED metachar. The menu must refuse
    // outright rather than fall back to the confirm prompt.
    env.write_jarvy_toml("[commands]\ndeploy = \"echo a && echo b\"\n");

    let mut s = env.spawn();
    s.expect("What would you like to do today?")
        .expect("menu prompt");

    s.send("deploy").expect("type filter");
    s.expect("Run `deploy`").expect("filtered option visible");
    s.send(ENTER).expect("select");

    s.expect("Refusing to run deploy command")
        .expect("hard refusal");
    // No "Execute this command?" prompt — the process exits directly
    // after the refusal, which Eof proves.
    let tail = s.expect(Eof).expect("process exits");
    // EP-3: the refusal path must NEVER fall back to the confirm prompt
    // AND must not execute either segment of the chained command. A bug
    // that printed the refusal AND still shelled out would print "a"
    // and "b" here — the negative assertions catch it.
    let tail_text = String::from_utf8_lossy(tail.before());
    assert!(
        !tail_text.contains("Execute this command?"),
        "hard-refused command still fell through to confirm prompt:\n{tail_text}",
    );
    assert!(
        !tail_text.contains("echo a") && !tail_text.contains("echo b"),
        "hard-refused command chain executed after refusal:\n{tail_text}",
    );
}

#[test]
fn safe_default_test_command_runs_without_confirmation() {
    let env = TestEnv::new();
    env.mark_initialized();
    // No jarvy.toml → the "Test the project" slot uses the safe
    // default `cargo test`, which is allowlisted: NO confirm prompt.

    let mut s = env.spawn();
    s.expect("What would you like to do today?")
        .expect("menu prompt");

    s.send("Test").expect("filter to test option");
    s.expect("Test the project")
        .expect("filtered option visible");
    s.send(ENTER).expect("select");

    // Straight to execution — the confirm gauntlet is skipped for
    // exact safe defaults. (cargo test then fails fast in the empty
    // project dir; the menu surfaces the exit code and moves on.)
    s.expect("Running test command: cargo test")
        .expect("executes without confirm prompt");
    s.expect(Eof).expect("process exits");
}
