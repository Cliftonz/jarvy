//! Integration tests for the `jarvy tools` CLI command.
//!
//! Tests the tool index generation and listing functionality.

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;
use tempfile::NamedTempFile;

/// Helper to run jarvy with test mode enabled
fn jarvy_cmd() -> Command {
    let mut c = Command::cargo_bin("jarvy").unwrap();
    c.env("JARVY_TEST_MODE", "1");
    c
}

#[test]
fn tools_list_default_pretty() {
    let mut c = jarvy_cmd();
    c.arg("tools");

    c.assert()
        .success()
        .stdout(predicate::str::contains("Supported tools"))
        .stdout(predicate::str::contains("git"))
        .stdout(predicate::str::contains("docker"));
}

#[test]
fn tools_list_json_format() {
    let mut c = jarvy_cmd();
    c.args(["tools", "--format", "json"]);

    let assert = c.assert().success();
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref()).to_string();

    // Verify it's valid JSON array
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("Should be valid JSON");
    assert!(parsed.is_array(), "Should be a JSON array");

    let arr = parsed.as_array().unwrap();
    assert!(!arr.is_empty(), "Should have tools");
    assert!(
        arr.iter().any(|v| v.as_str() == Some("git")),
        "Should contain git"
    );
}

#[test]
fn tools_list_yaml_format() {
    let mut c = jarvy_cmd();
    c.args(["tools", "--format", "yaml"]);

    c.assert()
        .success()
        .stdout(predicate::str::contains("- git"))
        .stdout(predicate::str::contains("- docker"));
}

#[test]
fn tools_index_json() {
    let mut c = jarvy_cmd();
    c.args(["tools", "--index", "--format", "json"]);

    let assert = c.assert().success();
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref()).to_string();

    // Verify it's valid JSON with expected structure
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("Should be valid JSON");

    assert!(parsed.get("version").is_some(), "Should have version field");
    assert!(parsed.get("count").is_some(), "Should have count field");
    assert!(parsed.get("tools").is_some(), "Should have tools field");

    let count = parsed["count"].as_u64().unwrap();
    let tools = parsed["tools"].as_array().unwrap();
    assert_eq!(
        count as usize,
        tools.len(),
        "count should match tools array length"
    );

    // Check that tools have required fields
    for tool in tools {
        assert!(tool.get("name").is_some(), "Each tool should have a name");
        assert!(
            tool.get("command").is_some(),
            "Each tool should have a command"
        );
        assert!(
            tool.get("custom_install").is_some(),
            "Each tool should have custom_install info"
        );
    }
}

#[test]
fn tools_index_pretty() {
    let mut c = jarvy_cmd();
    c.args(["tools", "--index"]);

    c.assert()
        .success()
        .stdout(predicate::str::contains("Tool Index v"))
        .stdout(predicate::str::contains("tools)"))
        .stdout(predicate::str::contains("git"))
        .stdout(predicate::str::contains("macOS"))
        .stdout(predicate::str::contains("Linux"));
}

#[test]
fn tools_index_contains_manual_tools() {
    let mut c = jarvy_cmd();
    c.args(["tools", "--index", "--format", "json"]);

    let assert = c.assert().success();
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref()).to_string();

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let tools = parsed["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();

    assert!(names.contains(&"nvm"), "Should contain nvm");
    assert!(names.contains(&"rust"), "Should contain rust");
    assert!(names.contains(&"brew"), "Should contain brew");
}

#[test]
fn tools_index_to_file() {
    let out = NamedTempFile::new().unwrap();
    let path = out.path().to_path_buf();
    drop(out);

    let mut c = jarvy_cmd();
    c.args(["tools", "--index", "--format", "json", "--output"])
        .arg(&path);

    let assert = c.assert().success();

    // stdout should be empty when writing to file
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref()).to_string();
    assert!(
        stdout.trim().is_empty(),
        "stdout should be empty with --output"
    );

    // File should contain valid JSON
    let content = std::fs::read_to_string(&path).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&content).expect("Output file should be valid JSON");
    assert!(parsed.get("tools").is_some());
}

#[test]
fn tools_list_has_minimum_tools() {
    let mut c = jarvy_cmd();
    c.args(["tools", "--format", "json"]);

    let assert = c.assert().success();
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref()).to_string();

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let arr = parsed.as_array().unwrap();

    // We know there should be at least these tools based on the codebase
    let expected_tools = vec![
        "git", "docker", "jq", "go", "python", "node", "rust", "brew",
    ];
    for expected in expected_tools {
        assert!(
            arr.iter().any(|v| v.as_str() == Some(expected)),
            "Should contain {}",
            expected
        );
    }
}

#[test]
fn tools_index_tools_are_sorted() {
    let mut c = jarvy_cmd();
    c.args(["tools", "--index", "--format", "json"]);

    let assert = c.assert().success();
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref()).to_string();

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let tools = parsed["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();

    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "Tools should be sorted alphabetically");
}
