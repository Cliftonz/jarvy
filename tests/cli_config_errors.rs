use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;
use tempfile::NamedTempFile;

#[test]
fn missing_config_exits_with_failure_and_message() {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.args(["get", "--file", "/definitely/missing/file.toml"]);
    // Error messages route to stderr so `--format json` stdout stays
    // parseable by `jq` pipelines. See review item 20.
    c.assert()
        .failure()
        .stderr(predicate::str::contains("Failed to read config file"));
}

#[test]
fn malformed_config_exits_with_failure_and_parse_message() {
    let tmp = NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), "[provisioner]\nthis = not_toml\n").unwrap();

    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1");
    c.args(["get", "--file"]).arg(tmp.path());
    c.assert()
        .failure()
        .stderr(predicate::str::contains("Failed to parse config file"));
}
