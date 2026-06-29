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
    c.assert()
        .success()
        .stdout(predicate::str::contains("TEST: user_select invoked"));

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
