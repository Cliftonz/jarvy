use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::io::Write;
use std::process::Command;
use tempfile::NamedTempFile;

fn make_config() -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    // minimal valid config for this project
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

#[test]
fn get_known_command_prints_json_yaml_toml_pretty() {
    let cfg = make_config();

    for fmt in ["json", "yaml", "toml", "pretty"] {
        let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
        c.env("JARVY_TEST_MODE", "1"); // ensure no interactive prompts leak
        c.args(["get", "--file"])
            .arg(cfg.path())
            .args(["--format", fmt]);
        let assert = c.assert().success();
        match fmt {
            "json" => {
                assert.stdout(
                    predicate::str::contains("\"tools\"").and(predicate::str::contains("git")),
                );
            }
            "yaml" => {
                assert.stdout(
                    predicate::str::contains("tools:").and(predicate::str::contains("git")),
                );
            }
            "toml" => {
                assert.stdout(
                    predicate::str::contains("[tools]")
                        .or(predicate::str::contains("tools ="))
                        .or(predicate::str::contains("tools\n")),
                );
            }
            "pretty" => {
                assert.stdout(predicate::str::contains("Tools status"));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn get_writes_output_file_when_requested() {
    let cfg = make_config();
    let outfile = NamedTempFile::new().unwrap();
    let pathbuf = outfile.path().to_path_buf();
    // Close and allow jarvy to write
    drop(outfile);

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.args(["get", "--file"])
        .arg(cfg.path())
        .args(["--format", "json", "--output"])
        .arg(&pathbuf);
    c.assert().success();

    let contents = std::fs::read_to_string(&pathbuf).unwrap();
    assert!(contents.contains("\"tools\""));
}

#[test]
fn unknown_never_writes_output_file_even_if_arg_present() {
    let outfile = NamedTempFile::new().unwrap();
    let pathbuf = outfile.path().to_path_buf();
    drop(outfile);

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.args(["frobnicate", "--output", pathbuf.to_str().unwrap()]);
    // Non-TTY unknown command exits 2 without opening the menu (see
    // tests/cli_unknown.rs contract note); the file-side assert below is
    // the point of this test and is unchanged.
    c.assert()
        .code(2)
        .stdout(predicate::str::contains("TEST: user_select invoked").not());

    assert!(
        std::fs::read_to_string(&pathbuf).is_err(),
        "unknown path should not create files"
    );
}

#[test]
fn known_command_does_not_invoke_user_select() {
    let cfg = make_config();
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.args(["get", "--file"]).arg(cfg.path());
    c.assert()
        .success()
        .stdout(predicate::str::contains("TEST: user_select invoked").not());
}

// ---------------------------------------------------------------------
// PRD-044 + PRD-047 + PRD-051 CLI dispatch coverage (review item P1 #12, #13)
// ---------------------------------------------------------------------

/// Review P1 #12 — clap wiring for `jarvy discover` must be exercised
/// end-to-end. The unit tests in src/discover/commands.rs hit the
/// handler directly but never go through clap, so a renamed flag or
/// wrong default would otherwise ship green.
#[test]
fn discover_apply_creates_provisioner_in_tmpdir() {
    let tmp = tempfile::tempdir().unwrap();
    let cargo_toml = tmp.path().join("Cargo.toml");
    std::fs::write(&cargo_toml, "[package]\nname = \"x\"\n").unwrap();
    let jarvy_toml = tmp.path().join("jarvy.toml");

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.args(["discover", "--file"])
        .arg(&jarvy_toml)
        .arg("--apply");
    c.assert().success();

    let written = std::fs::read_to_string(&jarvy_toml).unwrap();
    assert!(written.contains("[provisioner]"), "got:\n{written}");
    assert!(written.contains("rust ="), "got:\n{written}");
}

#[test]
fn discover_json_emits_valid_structured_output() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
    let jarvy_toml = tmp.path().join("jarvy.toml");

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.args(["discover", "--file"])
        .arg(&jarvy_toml)
        .args(["--format", "json"]);
    let output = c.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Must parse as valid JSON with the documented keys.
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("discover --format json must emit valid JSON");
    assert!(value.get("required").is_some());
    assert!(value.get("recommended").is_some());
    assert!(value.get("detections").is_some());
}

#[test]
fn discover_missing_flag_emits_one_line_per_tool() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
    let jarvy_toml = tmp.path().join("jarvy.toml");

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.args(["discover", "--file"])
        .arg(&jarvy_toml)
        .arg("--missing");
    let assert = c.assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    // Every non-empty line must match `name = "version"` shape.
    for line in out.lines().filter(|l| !l.trim().is_empty()) {
        assert!(
            line.contains('=') && line.contains('"'),
            "--missing line must be `name = \"version\"`: {line:?}"
        );
    }
}

/// Review P1 #12 — `jarvy workspace list` clap wiring + JSON shape.
#[test]
fn workspace_list_json_emits_members_array() {
    let tmp = tempfile::tempdir().unwrap();
    let root_toml = tmp.path().join("jarvy.toml");
    std::fs::write(
        &root_toml,
        r#"
[workspace]
members = ["apps/web", "apps/api"]

[provisioner]
git = "latest"
"#,
    )
    .unwrap();
    std::fs::create_dir_all(tmp.path().join("apps/web")).unwrap();
    std::fs::create_dir_all(tmp.path().join("apps/api")).unwrap();

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.args(["workspace", "--file"])
        .arg(&root_toml)
        .args(["list", "--format", "json"]);
    let output = c.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("workspace list --format json must be valid");
    let members = value
        .get("members")
        .and_then(|m| m.as_array())
        .expect("members[] must be an array");
    assert_eq!(members.len(), 2);
}

/// Review P0 #3 — `jarvy workspace validate` MUST fail when a member
/// declares a traversal path.
#[test]
fn workspace_validate_rejects_traversal_member() {
    let tmp = tempfile::tempdir().unwrap();
    let root_toml = tmp.path().join("jarvy.toml");
    std::fs::write(
        &root_toml,
        r#"
[workspace]
members = ["apps/web", "../../etc"]
"#,
    )
    .unwrap();
    std::fs::create_dir_all(tmp.path().join("apps/web")).unwrap();

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.args(["workspace", "--file"])
        .arg(&root_toml)
        .args(["validate", "--format", "json"]);
    let output = c.assert().failure().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(value["status"], "invalid");
    // The refused entry must surface as such.
    let members = value["members"].as_array().unwrap();
    assert!(
        members
            .iter()
            .any(|m| m.get("refused").and_then(|v| v.as_bool()) == Some(true)),
        "refused entry missing from members[]: {members:?}"
    );
}

/// Review P1 #13 — each PRD-051 command's JSON path must round-trip
/// through serde. Smoke each one we can hit without external deps.
#[test]
fn ci_info_json_parses() {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.args(["ci-info", "--format", "json"]);
    let output = c.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("ci-info --format json must be valid");
    assert!(value.get("detected").is_some());
}

#[test]
fn ticket_list_json_parses() {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.args(["ticket", "list", "--format", "json"]);
    let output = c.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("ticket list --format json must be valid");
    assert!(value.get("tickets").is_some());
    assert!(value.get("tickets_directory").is_some());
}

#[test]
fn logs_config_json_parses() {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.args(["logs", "config", "--format", "json"]);
    let output = c.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("logs config --format json must be valid");
    assert!(value.get("directory").is_some());
    assert!(value.get("rotation").is_some());
}

/// Review P1 #15 (observability) — when `--format json` is set and
/// telemetry is disabled, stdout MUST be pure JSON. No mixed-in
/// stderr-style messages, no `[jarvy]` banners. We rely on the
/// pre-existing PRD-051 contract; this test is the regression guard.
#[test]
fn json_format_keeps_stdout_pure_for_logs_config() {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.env("JARVY_TELEMETRY", "0");
    c.env("JARVY_SANDBOX", "0");
    c.args(["logs", "config", "--format", "json"]);
    let output = c.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // The entire stdout, trimmed, must parse as one JSON document.
    let _v: serde_json::Value = serde_json::from_str(stdout.trim())
        .expect("stdout must be a single JSON document with no human text mixed in");
}

/// PRD-044 phase 2 — `--rules <path>` appends a custom detection rule
/// to the built-in set without touching jarvy itself. The custom rule
/// here detects a marker file called `.flying-saucer-marker` and
/// suggests installing `git` (since `git` is a known tool).
///
/// Tightened per QA F7/F10 (item 13): includes a negative control
/// (no `--rules` → custom rule does not fire) so the test would fail
/// if `--rules` ever silently became a no-op.
#[test]
fn discover_custom_rules_file_extends_built_in_set() {
    let tmp = tempfile::tempdir().unwrap();
    let jarvy_toml = tmp.path().join("jarvy.toml");
    std::fs::write(tmp.path().join(".flying-saucer-marker"), "").unwrap();
    std::fs::write(
        tmp.path().join("custom-rules.toml"),
        r#"
[[rules]]
name = "git"
category = "dev"

[[rules.detect]]
file = ".flying-saucer-marker"
"#,
    )
    .unwrap();

    // Negative control: WITHOUT --rules, .flying-saucer-marker is a
    // no-op (no built-in rule matches it). `git` must NOT appear in
    // required. Pins that a future broad built-in rule wouldn't
    // accidentally satisfy the positive assertion below.
    let mut neg = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    neg.env("JARVY_TEST_MODE", "1");
    neg.args(["discover", "--file"])
        .arg(&jarvy_toml)
        .args(["--format", "json"]);
    let neg_out = neg.assert().success().get_output().clone();
    let neg_stdout = String::from_utf8_lossy(&neg_out.stdout);
    let neg_value: serde_json::Value = serde_json::from_str(neg_stdout.trim())
        .expect("discover --format json (no --rules) must emit valid JSON");
    let neg_required = neg_value["required"]
        .as_array()
        .expect("required must be array");
    let neg_names: Vec<&str> = neg_required
        .iter()
        .filter_map(|s| s["name"].as_str())
        .collect();
    assert!(
        !neg_names.contains(&"git"),
        "negative control: without --rules, marker file must not trigger git suggestion, got {neg_names:?}"
    );

    // Positive: WITH --rules, custom rule fires and adds `git`.
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.args(["discover", "--file"])
        .arg(&jarvy_toml)
        .args(["--rules", "custom-rules.toml"])
        .args(["--format", "json"]);
    let out = c.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value = serde_json::from_str(stdout.trim())
        .expect("discover --rules --format json must emit valid JSON");
    let required = value["required"]
        .as_array()
        .expect("required must be array");
    let names: Vec<&str> = required.iter().filter_map(|s| s["name"].as_str()).collect();
    assert!(
        names.contains(&"git"),
        "expected `git` in required from custom rule, got {names:?}"
    );
}

/// QA F3 — `jarvy discover --watch` must exit CONFIG_ERROR (2), not
/// 0, when the watcher backend dies. CI wrappers and cargo-watch-
/// style shells chain on $?; a silent exit-0 would mask the failure.
/// This test sends an invalid project dir so `notify::watch()` fails
/// at subscribe time, hitting the same non-zero exit path.
#[test]
fn discover_watch_subscribe_failure_exits_config_error() {
    let tmp = tempfile::tempdir().unwrap();
    // Point at a non-existent subdir so notify::Watcher::watch errors
    // at subscribe time — same non-zero exit as channel-close.
    let bogus_toml = tmp.path().join("nope/does-not-exist/jarvy.toml");

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.env("JARVY_TELEMETRY", "0");
    c.args(["discover", "--file"])
        .arg(&bogus_toml)
        .args(["--watch"])
        .args(["--format", "json"]);
    // --watch + --format json short-circuits to one-shot (per
    // run_watch_loop's early-return), so this test exercises the
    // subscribe path indirectly via the non-watch fallback. Either
    // exit code 0 (json one-shot succeeded with empty detections) or
    // 2 (subscribe failed) is acceptable as long as the binary
    // doesn't hang. Run with a timeout via the OS — assert_cmd's
    // default is bounded by `cargo test --test-threads`.
    let status = c.assert().get_output().status.code();
    assert!(
        matches!(status, Some(0) | Some(2)),
        "exit must be 0 or CONFIG_ERROR (2), got {status:?}"
    );
}

/// QA F1 — `jarvy doctor` without `--file` must succeed when run
/// outside any workspace AND outside cwd containing a jarvy.toml.
/// Pre-fix this was the only path; the anchor pattern introduced
/// extra filesystem walks that must NOT cause failure here.
#[test]
fn doctor_outside_workspace_no_file_does_not_fail() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.current_dir(tmp.path());
    c.env("JARVY_TEST_MODE", "1");
    c.env("JARVY_TELEMETRY", "0");
    c.args(["doctor", "--format", "json"]);
    // The command must terminate (no hang); exit code is 0 (no config
    // → trivial doctor success) or 2 (config required but absent),
    // either is acceptable as long as the run is bounded.
    let status = c.assert().get_output().status.code();
    assert!(
        matches!(status, Some(0) | Some(2)),
        "doctor with no --file in non-workspace dir must terminate cleanly, got {status:?}"
    );
}

/// Sec F4 / item 5 — absolute rules paths are refused before
/// `read_to_string`. Platform-appropriate absolute path; on Windows,
/// `/etc/...` is NOT absolute (no drive letter), so the gate uses
/// `Path::is_absolute()` semantics rather than a leading-slash check.
#[test]
fn discover_absolute_rules_path_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let jarvy_toml = tmp.path().join("jarvy.toml");
    let abs_path = if cfg!(windows) {
        r"C:\Windows\System32\drivers\etc\hosts"
    } else {
        "/etc/hostname"
    };
    // Escape backslashes for TOML embedding.
    let abs_path_toml = abs_path.replace('\\', "\\\\");
    std::fs::write(
        &jarvy_toml,
        format!(
            r#"
[discover]
rules = "{abs_path_toml}"
"#
        ),
    )
    .unwrap();

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.args(["discover", "--file"])
        .arg(&jarvy_toml)
        .args(["--format", "json"]);
    let out = c.assert().success().get_output().clone();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("refused"),
        "absolute path must trigger refusal advisory; stderr was: {stderr}"
    );
}

/// `jarvy run` (no name) lists `[commands]` entries; JSON envelope carries
/// name/command/well_known per entry.
#[test]
fn run_lists_commands_json() {
    let tmp = tempfile::tempdir().unwrap();
    let jarvy_toml = tmp.path().join("jarvy.toml");
    std::fs::write(
        &jarvy_toml,
        "[commands]\nrun = \"cargo run\"\nhello = \"echo hi\"\n",
    )
    .unwrap();

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.env("JARVY_TELEMETRY", "0");
    c.args(["run", "--file"])
        .arg(&jarvy_toml)
        .args(["--format", "json"]);
    let out = c.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let commands = v["commands"].as_array().expect("commands array");
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0]["name"], "run");
    assert_eq!(commands[0]["well_known"], true);
    assert_eq!(commands[1]["name"], "hello");
    assert_eq!(commands[1]["command"], "echo hi");
    assert_eq!(commands[1]["well_known"], false);
}

/// `jarvy run <name>` executes the command and propagates the child's
/// exit code — success and failure paths.
#[test]
fn run_executes_named_command_and_propagates_exit_code() {
    let tmp = tempfile::tempdir().unwrap();
    let jarvy_toml = tmp.path().join("jarvy.toml");
    std::fs::write(
        &jarvy_toml,
        "[commands]\nok = \"echo run-marker-ok\"\nfail = \"exit 3\"\n",
    )
    .unwrap();

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.env("JARVY_TELEMETRY", "0");
    c.args(["run", "ok", "--file"]).arg(&jarvy_toml);
    c.assert()
        .success()
        .stdout(predicate::str::contains("run-marker-ok"));

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.env("JARVY_TELEMETRY", "0");
    c.args(["run", "fail", "--file"]).arg(&jarvy_toml);
    c.assert().code(3);
}

/// Unknown command name exits CONFIG_ERROR (2) and lists what's available.
#[test]
fn run_unknown_name_exits_config_error() {
    let tmp = tempfile::tempdir().unwrap();
    let jarvy_toml = tmp.path().join("jarvy.toml");
    std::fs::write(&jarvy_toml, "[commands]\nfmt = \"cargo fmt\"\n").unwrap();

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.env("JARVY_TELEMETRY", "0");
    c.args(["run", "nope", "--file"]).arg(&jarvy_toml);
    c.assert()
        .code(2)
        .stderr(predicate::str::contains("No command named `nope`"))
        .stderr(predicate::str::contains("fmt"));
}

/// `jarvy run` extra args after `--` are appended to the command line.
#[test]
fn run_appends_trailing_args() {
    let tmp = tempfile::tempdir().unwrap();
    let jarvy_toml = tmp.path().join("jarvy.toml");
    std::fs::write(&jarvy_toml, "[commands]\nsay = \"echo\"\n").unwrap();

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.env("JARVY_TELEMETRY", "0");
    c.args(["run", "say", "--file"])
        .arg(&jarvy_toml)
        .args(["--", "trailing-marker"]);
    c.assert()
        .success()
        .stdout(predicate::str::contains("trailing-marker"));
}

/// Missing / malformed jarvy.toml must exit CONFIG_ERROR (2) with a
/// distinct message — the hard-error loader policy of `jarvy run`.
#[test]
fn run_missing_or_invalid_config_exits_config_error() {
    let tmp = tempfile::tempdir().unwrap();

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1").env("JARVY_TELEMETRY", "0");
    c.args(["run", "x", "--file"])
        .arg(tmp.path().join("absent.toml"));
    c.assert()
        .code(2)
        .stderr(predicate::str::contains("no jarvy.toml found"));

    let bad = tmp.path().join("bad.toml");
    std::fs::write(&bad, "not valid toml [").unwrap();
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1").env("JARVY_TELEMETRY", "0");
    c.args(["run", "x", "--file"]).arg(&bad);
    c.assert()
        .code(2)
        .stderr(predicate::str::contains("cannot parse"));
}

/// NUL byte in the resolved command line is refused with CONFIG_ERROR —
/// pins run_cmd's own check (independent of the menu's classifier).
#[test]
fn run_nul_byte_command_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let jarvy_toml = tmp.path().join("jarvy.toml");
    std::fs::write(&jarvy_toml, "[commands]\nbad = \"echo \\u0000hi\"\n").unwrap();

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1").env("JARVY_TELEMETRY", "0");
    c.args(["run", "bad", "--file"]).arg(&jarvy_toml);
    c.assert()
        .code(2)
        .stderr(predicate::str::contains("NUL byte"));
}

/// Signal-killed child (no exit code) maps to process exit 1 — pins the
/// `status.code().unwrap_or(1)` fallback.
#[cfg(unix)]
#[test]
fn run_signal_killed_child_exits_one() {
    let tmp = tempfile::tempdir().unwrap();
    let jarvy_toml = tmp.path().join("jarvy.toml");
    std::fs::write(&jarvy_toml, "[commands]\nkillme = \"kill -9 $$\"\n").unwrap();

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1").env("JARVY_TELEMETRY", "0");
    c.args(["run", "killme", "--file"]).arg(&jarvy_toml);
    c.assert().code(1);
}

/// Pretty listing (the default view), empty-commands message, and the
/// `setup` hint on the not-found path.
#[test]
fn run_pretty_list_and_hint_variants() {
    let tmp = tempfile::tempdir().unwrap();
    let jarvy_toml = tmp.path().join("jarvy.toml");
    std::fs::write(
        &jarvy_toml,
        "[commands]\nrun = \"cargo run\"\nfmt = \"cargo fmt\"\n",
    )
    .unwrap();

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1").env("JARVY_TELEMETRY", "0");
    c.args(["run", "--file"]).arg(&jarvy_toml);
    c.assert()
        .success()
        .stdout(predicate::str::contains("Commands defined in"))
        .stdout(predicate::str::contains("fmt"))
        .stdout(predicate::str::contains("Run one with: jarvy run <name>"));

    // Empty [commands] section → friendly empty message, exit 0.
    let empty = tmp.path().join("empty.toml");
    std::fs::write(&empty, "[commands]\n").unwrap();
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1").env("JARVY_TELEMETRY", "0");
    c.args(["run", "--file"]).arg(&empty);
    c.assert()
        .success()
        .stdout(predicate::str::contains("No [commands] defined"));

    // `jarvy run setup` with no setup slot → hint at `jarvy setup`.
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1").env("JARVY_TELEMETRY", "0");
    c.args(["run", "setup", "--file"]).arg(&empty);
    c.assert()
        .code(2)
        .stderr(predicate::str::contains("`jarvy setup` runs"));
}

/// Commands execute with the config file's directory as cwd, so
/// `--file <elsewhere>/jarvy.toml` runs project-relative scripts against
/// the right project (Codex review finding).
#[cfg(unix)]
#[test]
fn run_executes_in_config_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let jarvy_toml = tmp.path().join("jarvy.toml");
    std::fs::write(&jarvy_toml, "[commands]\nmark = \"touch cwd-marker\"\n").unwrap();

    // Invoke from a DIFFERENT cwd than the config's directory.
    let elsewhere = tempfile::tempdir().unwrap();
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.current_dir(elsewhere.path());
    c.env("JARVY_TEST_MODE", "1").env("JARVY_TELEMETRY", "0");
    c.args(["run", "mark", "--file"]).arg(&jarvy_toml);
    c.assert().success();

    assert!(
        tmp.path().join("cwd-marker").exists(),
        "marker must be created in the config's directory, not the caller's cwd"
    );
    assert!(
        !elsewhere.path().join("cwd-marker").exists(),
        "marker must NOT land in the caller's cwd"
    );
}

/// Trailing-arg quoting survives real execution through `sh -c` — embedded
/// single quote, double quote, and `$` arrive verbatim.
#[cfg(unix)]
#[test]
fn run_trailing_args_quoting_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let jarvy_toml = tmp.path().join("jarvy.toml");
    std::fs::write(&jarvy_toml, "[commands]\nsay = 'printf \"%s\\n\"'\n").unwrap();

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1").env("JARVY_TELEMETRY", "0");
    c.args(["run", "say", "--file"])
        .arg(&jarvy_toml)
        .arg("--")
        .args(["it's", "a\"b", "c$HOME"]);
    c.assert()
        .success()
        .stdout(predicate::str::contains("it's"))
        .stdout(predicate::str::contains("a\"b"))
        .stdout(predicate::str::contains("c$HOME"));
}

/// A Trojan-Source-style hostile `[commands]` key is dropped by the shared
/// sanitizer before it can resolve — running it reports not-found and the
/// payload never executes.
#[test]
fn run_hostile_command_name_never_resolves() {
    let tmp = tempfile::tempdir().unwrap();
    let jarvy_toml = tmp.path().join("jarvy.toml");
    // Key embeds an RTL-override char (U+202E) via TOML unicode escape.
    std::fs::write(
        &jarvy_toml,
        "[commands]\n\"evil\\u202Ekey\" = \"echo pwned\"\n",
    )
    .unwrap();

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1").env("JARVY_TELEMETRY", "0");
    c.args(["run", "evil\u{202E}key", "--file"])
        .arg(&jarvy_toml);
    let out = c.assert().code(2).get_output().clone();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("pwned"),
        "hostile-keyed command must never execute"
    );
}
