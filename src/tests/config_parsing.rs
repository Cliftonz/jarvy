use crate::config::Config;

#[test]
fn test_parse_simple_config() {
    let toml_content = r#"
    [tools]
    git = "latest"
    "#;

    let config: Config = toml::from_str(toml_content).expect("Failed to parse TOML");

    let tools = config.get_tool_configs();

    assert_eq!(tools["git"].version, "latest");
    // assert_eq!(tools["git"].package_manager, true);
}

#[test]
fn test_parse_complex_config() {
    let toml_content = r#"
    [tools]
    node = { version = "14.15.0", package_manager = true }
    "#;

    let config: Config = toml::from_str(toml_content).expect("Failed to parse TOML");

    let tools = config.get_tool_configs();

    assert_eq!(tools["node"].version, "14.15.0");
    // assert_eq!(tools["node"].package_manager, true);
}

#[test]
fn test_parse_mixed_config() {
    let toml_content = r#"
    [tools]
    git = "latest"
    node = { version = "14.15.0", package_manager = true }
    python3 = { version = "3.9.0", package_manager = false }
    docker = "latest"
    "#;

    let config: Config = toml::from_str(toml_content).expect("Failed to parse TOML");

    let tools = config.get_tool_configs();

    assert_eq!(tools["git"].version, "latest");
    // assert_eq!(tools["git"].package_manager, true);
    assert_eq!(tools["node"].version, "14.15.0");
    // assert_eq!(tools["node"].package_manager, true);
    assert_eq!(tools["python3"].version, "3.9.0");
    // assert_eq!(tools["python3"].package_manager, false);
    assert_eq!(tools["docker"].version, "latest");
    // assert_eq!(tools["docker"].package_manager, true);
}
