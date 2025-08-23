use assert_cmd::Command;
use tempfile::NamedTempFile;
use std::fs::write;

#[test]
fn get_command_outputs_json() {
    let toml_content = r#"
    [provisioner]
    rustc = "latest"
    "#;

    let file = NamedTempFile::new().unwrap();
    write(file.path(), toml_content).unwrap();

    let mut cmd = Command::cargo_bin("jarvy").unwrap();
    cmd.arg("get")
        .arg("--file").arg(file.path())
        .arg("--format").arg("json");

    let assert = cmd.assert().success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).unwrap();

    assert!(output.contains("rustc"));
    assert!(output.contains("\"status\":\"match\""));
}
