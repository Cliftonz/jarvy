// use assert_cmd::Command;
// use tempfile::NamedTempFile;
// use std::fs::write;
//
// #[test]
// fn test_cli_default_config() {
//     let mut cmd = Command::cargo_bin("jarvy").unwrap();
//     cmd.arg("setup");
//     cmd.assert().failure().stderr(predicates::str::contains("Failed to read config file"));
// }
//
// #[test]
// fn test_cli_custom_config() {
//     let toml_content = r#"
//     [provisioner]
//     git = "latest"
//     node = { version = "14.15.0", package_manager = true }
//     python3 = { version = "3.9.0", package_manager = false }
//     docker = "latest"
//     "#;
//
//     let file = NamedTempFile::new().unwrap();
//     write(file.path(), toml_content).unwrap();
//
//     let mut cmd = Command::cargo_bin("jarvy").unwrap();
//     cmd.arg("setup").arg("--file").arg(file.path());
//     cmd.assert().success().stdout(predicates::str::contains("Installing git version latest using package manager: true"));
//     cmd.assert().success().stdout(predicates::str::contains("Installing node version 14.15.0 using package manager: true"));
//     cmd.assert().success().stdout(predicates::str::contains("Installing python3 version 3.9.0 using package manager: false"));
//     cmd.assert().success().stdout(predicates::str::contains("Installing docker version latest using package manager: true"));
// }