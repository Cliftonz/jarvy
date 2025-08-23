use crate::config::Config;
use crate::report::{Status, collect_reports};

fn build_config(toml_content: &str) -> Config {
    toml::from_str::<Config>(toml_content).expect("Failed to parse TOML for test config")
}

#[test]
fn test_collect_reports_statuses() {
    // Use commands that are deterministic in CI: cargo and rustc should exist; a bogus one should not.
    let cfg = build_config(
        r#"
        [provisioner]
        rustc = "definitely-not-a-real-version-prefix"
        cargo = "latest"
        not_a_real_command_abcdef = "1.2.3"
        "#,
    );

    let reports = collect_reports(&cfg);

    // Convert to map for stable assertions
    use std::collections::HashMap;
    let by_name: HashMap<_, _> = reports.into_iter().map(|r| (r.name.clone(), r)).collect();

    // rustc should be installed, but with a version not containing the expected string -> Mismatch
    let rustc = by_name.get("rustc").expect("rustc report missing");
    assert_eq!(rustc.status, Status::Mismatch);
    assert!(
        rustc.installed.as_ref().is_some(),
        "rustc should be installed in CI"
    );

    // cargo expected latest -> Match regardless of version string
    let cargo = by_name.get("cargo").expect("cargo report missing");
    assert_eq!(cargo.status, Status::Match);
    assert!(
        cargo.installed.as_ref().is_some(),
        "cargo should be installed in CI"
    );

    // bogus command -> NotInstalled, installed is None
    let bogus = by_name
        .get("not_a_real_command_abcdef")
        .expect("bogus report missing");
    assert_eq!(bogus.status, Status::NotInstalled);
    assert!(bogus.installed.is_none());
}

#[test]
fn test_serialization_json_yaml_status_strings() {
    let cfg = build_config(
        r#"
        [provisioner]
        rustc = "no-match"
        cargo = "latest"
        nope_cmd_xxx = "0.0.0"
        "#,
    );

    let reports = collect_reports(&cfg);

    // JSON serialization
    let json_str = serde_json::to_string(&reports).expect("json serialize");
    // Ensure snake_case status strings appear
    assert!(json_str.contains("\"match\""));
    assert!(json_str.contains("\"mismatch\""));
    assert!(json_str.contains("\"not_installed\""));

    // Also round-trip a small check using Value for fields presence
    let v: serde_json::Value = serde_json::from_str(&json_str).expect("json parse");
    assert!(v.is_array());
    for item in v.as_array().unwrap() {
        assert!(item.get("name").is_some());
        assert!(item.get("expected").is_some());
        assert!(item.get("status").is_some());
        // installed can be null or string
        assert!(item.get("installed").is_some());
    }

    // YAML serialization
    let yaml_str = serde_yaml::to_string(&reports).expect("yaml serialize");
    // Status strings are the same values in YAML
    assert!(yaml_str.contains("match"));
    assert!(yaml_str.contains("mismatch"));
    assert!(yaml_str.contains("not_installed"));
}
