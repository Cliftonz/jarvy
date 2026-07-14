use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

fn cmd() -> Command {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c
}

#[test]
fn help_shows_usage_and_exits_zero() {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.arg("--help");
    c.assert()
        .success()
        .stdout(predicate::str::contains("Usage").or(predicate::str::contains("USAGE")));
}

#[test]
fn version_exits_zero() {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.arg("-V");
    c.assert().success();
}

// Unknown-command contract (changed after the release-paths incident where
// `jarvy rollback` — no such command — exited 0 in CI and only a downstream
// version assert caught it): with no TTY, exit 2 immediately and do NOT open
// the interactive menu, regardless of JARVY_TEST_MODE. The menu fallback is
// TTY-only. Tests run without a TTY, so they observe the exit-2 path.
#[test]
fn unknown_triggers_handler_and_no_init() {
    let mut c = cmd();
    c.env("JARVY_INIT_PROBE", "1");
    c.arg("frobnicate");
    c.assert()
        .code(2)
        .stderr(predicate::str::contains(
            "Unrecognized command: 'frobnicate'",
        ))
        .stderr(predicate::str::contains("TEST: initialize called").not())
        .stdout(predicate::str::contains("TEST: user_select invoked").not());
}

#[test]
fn no_command_invokes_interactive_after_init() {
    let mut c = cmd();
    c.env("JARVY_INIT_PROBE", "1");
    c.assert()
        .success()
        .stderr(predicate::str::contains("TEST: initialize called"))
        .stdout(predicate::str::contains("TEST: user_select invoked"));
}

#[test]
fn unknown_plus_known_like_args_falls_back() {
    let mut c = cmd();
    c.env("JARVY_INIT_PROBE", "1");
    c.args(["z", "--format", "json"]);
    c.assert()
        .code(2)
        .stderr(predicate::str::contains("Unrecognized command: 'z'"))
        .stdout(predicate::str::contains("TEST: user_select invoked").not());
}

#[test]
fn case_mismatch_subcommand_is_unknown() {
    let mut c = cmd();
    c.env("JARVY_INIT_PROBE", "1");
    c.args(["SeTuP"]);
    c.assert()
        .code(2)
        // unknown path returns before init
        .stderr(predicate::str::contains("TEST: initialize called").not())
        .stdout(predicate::str::contains("TEST: user_select invoked").not());
}

#[test]
fn negative_malformed_get_flag_errors() {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.args(["get", "--unknown-flag"]);
    c.assert()
        .failure()
        .stderr(predicate::str::contains("error").or(predicate::str::contains("Usage")));
}

#[test]
fn negative_invalid_format_variant_errors() {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.args(["get", "--format", "frobnicate"]);
    c.assert().failure().stderr(
        predicate::str::contains("possible values").or(predicate::str::contains("valid values")),
    );
}
