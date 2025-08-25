use crate::config::Config;

#[test]
fn test_parse_simple_config() {
    let toml_content = r#"
    [provisioner]
    git = "latest"
    "#;

    let config: Config = toml::from_str(toml_content).expect("Failed to parse TOML");

    let tools = config.get_tool_configs();

    println!("{:?}", tools);

    let git = tools.get("git").expect("expected 'git' tool in config");
    assert_eq!(git.version, "latest");
    assert!(git.version_manager);
    assert_eq!(git.use_sudo, None);
}

#[test]
fn test_parse_complex_config() {
    let toml_content = r#"
    [provisioner]
    node = { version = "14.15.0", version_manager = false, use_sudo = true }
    "#;

    let config: Config = toml::from_str(toml_content).expect("Failed to parse TOML");

    let tools = config.get_tool_configs();

    let node = tools.get("node").expect("expected 'node' tool in config");
    assert_eq!(node.version, "14.15.0");
    assert!(!node.version_manager);
    assert_eq!(node.use_sudo, Some(true));
}

#[test]
fn test_parse_mixed_config() {
    let toml_content = r#"
    [provisioner]
    git = "latest"
    node = { version = "14.15.0", version_manager = true, use_sudo = false }
    python3 = { version = "3.9.0", version_manager = false, use_sudo = true }
    docker = "latest"
    "#;

    let config: Config = toml::from_str(toml_content).expect("Failed to parse TOML");

    let tools = config.get_tool_configs();

    let git = tools.get("git").expect("expected 'git' tool in config");
    assert_eq!(git.version, "latest");
    assert!(git.version_manager);
    assert_eq!(git.use_sudo, None);

    let node = tools.get("node").expect("expected 'node' tool in config");
    assert_eq!(node.version, "14.15.0");
    assert!(node.version_manager);
    assert_eq!(node.use_sudo, Some(false));

    let py3 = tools
        .get("python3")
        .expect("expected 'python3' tool in config");
    assert_eq!(py3.version, "3.9.0");
    assert!(!py3.version_manager);
    assert_eq!(py3.use_sudo, Some(true));
}
