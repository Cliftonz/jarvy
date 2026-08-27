use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

/// Minimal valid `jarvy.toml` body, matching the shape other CLI
/// integration tests use.
const MINIMAL_CONFIG: &str = "[provisioner]\ngit = \"latest\"\n";

/// `jarvy get` (default `--file`, none passed) from a subdirectory finds
/// the `jarvy.toml` sitting at the search boundary (the JARVY_HOME root
/// itself) and succeeds, mirroring `git`'s upward walk to find `.git`.
#[test]
fn default_file_searches_parent_directories() {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(home.path().join("jarvy.toml"), MINIMAL_CONFIG).unwrap();
    let sub = home.path().join("sub").join("dir");
    std::fs::create_dir_all(&sub).unwrap();

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.current_dir(&sub);
    c.env("JARVY_TEST_MODE", "1");
    c.env("JARVY_TELEMETRY", "0");
    c.env("JARVY_HOME", home.path());
    c.args(["get", "--format", "json"]);
    c.assert().success();
}

/// An explicit `--file` is consent to use exactly that path; it must
/// never trigger the upward search, even from the same subdirectory
/// tree that a default-path search would resolve successfully.
#[test]
fn explicit_file_never_searches_parents() {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(home.path().join("jarvy.toml"), MINIMAL_CONFIG).unwrap();
    let sub = home.path().join("sub").join("dir");
    std::fs::create_dir_all(&sub).unwrap();

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.current_dir(&sub);
    c.env("JARVY_TEST_MODE", "1");
    c.env("JARVY_TELEMETRY", "0");
    c.env("JARVY_HOME", home.path());
    c.args(["get", "--file", "custom.toml"]);
    c.assert()
        .failure()
        .stderr(predicate::str::contains("Failed to read config file"));
}

/// The chdir into the discovered root is real, not just config parsing:
/// `jarvy run <name>` (no `--file`) resolves `[commands]` from the
/// parent-found `jarvy.toml` AND executes with that directory as cwd.
#[test]
fn run_command_executes_after_parent_search_chdir() {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(
        home.path().join("jarvy.toml"),
        "[commands]\nbuild = \"echo parent-search-marker\"\n",
    )
    .unwrap();
    let sub = home.path().join("sub").join("dir");
    std::fs::create_dir_all(&sub).unwrap();

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.current_dir(&sub);
    c.env("JARVY_TEST_MODE", "1");
    c.env("JARVY_TELEMETRY", "0");
    c.env("JARVY_HOME", home.path());
    c.args(["run", "build"]);
    c.assert()
        .success()
        .stdout(predicate::str::contains("parent-search-marker"));
}

/// Security F2 regression: a `jarvy.toml` sitting ABOVE the JARVY_HOME
/// boundary must never be found, even though it's a real ancestor
/// directory of cwd on disk. The walk refuses outright because cwd
/// doesn't canonicalize under home at all.
#[test]
fn search_refuses_to_climb_above_home_boundary() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("jarvy.toml"), MINIMAL_CONFIG).unwrap();

    let home = root.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let outside = root.path().join("outside").join("sub");
    std::fs::create_dir_all(&outside).unwrap();

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.current_dir(&outside);
    c.env("JARVY_TEST_MODE", "1");
    c.env("JARVY_TELEMETRY", "0");
    c.env("JARVY_HOME", &home);
    c.args(["get", "--format", "json"]);
    c.assert()
        .failure()
        .stderr(predicate::str::contains("Failed to read config file"));
}
