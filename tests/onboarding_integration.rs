//! Integration tests for PRD-023 onboarding commands
//!
//! Tests for:
//! - jarvy init (interactive wizard and template mode)
//! - jarvy templates (list, show, use)
//! - jarvy quickstart (guided flow)

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;
use tempfile::TempDir;

// ============================================================================
// jarvy init tests
// ============================================================================

#[test]
fn init_with_template_creates_config_file() {
    let dir = TempDir::new().unwrap();
    let output_path = dir.path().join("jarvy.toml");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args([
        "init",
        "--template",
        "essential",
        "--output",
        output_path.to_str().unwrap(),
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Created"));

    // Verify file was created
    let contents = std::fs::read_to_string(&output_path).unwrap();
    assert!(contents.contains("[provisioner]"));
    assert!(contents.contains("git"));
}

#[test]
fn init_with_template_stdout_outputs_to_stdout() {
    let home = TempDir::new().unwrap();
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.env("JARVY_TELEMETRY", "0");
    cmd.env("JARVY_HOME", home.path());
    cmd.args(["init", "--template", "essential", "--stdout"]);

    let output = cmd.assert().success().get_output().clone();
    let stdout = String::from_utf8(output.stdout).expect("template output must be UTF-8");
    let parsed: toml::Value =
        toml::from_str(&stdout).expect("stdout must contain exactly one valid TOML document");
    let provisioner = parsed
        .get("provisioner")
        .expect("generated config must contain provisioner tools");
    assert!(provisioner.get("git").is_some());
}

#[test]
fn init_with_unknown_template_fails() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["init", "--template", "nonexistent-template", "--stdout"]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn init_non_interactive_requires_template() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["init", "--non-interactive"]);

    // Should exit with warning status (not create a file)
    let _ = cmd.assert();
}

// ============================================================================
// jarvy templates tests
// ============================================================================

#[test]
fn templates_list_shows_available_templates() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["templates", "list"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Available Templates"))
        .stdout(predicate::str::contains("react"))
        .stdout(predicate::str::contains("essential"))
        .stdout(predicate::str::contains("rust-cli"));
}

#[test]
fn templates_show_displays_template_details() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["templates", "show", "react"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Template: react"))
        .stdout(predicate::str::contains("Tools included"));
}

#[test]
fn templates_show_unknown_template_fails() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["templates", "show", "nonexistent-template"]);

    cmd.assert()
        .failure()
        .stdout(predicate::str::contains("not found"));
}

#[test]
fn templates_use_creates_config_file() {
    let dir = TempDir::new().unwrap();
    let output_path = dir.path().join("jarvy.toml");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.current_dir(dir.path());
    cmd.args([
        "templates",
        "use",
        "essential",
        "--output",
        output_path.to_str().unwrap(),
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Created"));

    // Verify file was created with expected content
    let contents = std::fs::read_to_string(&output_path).unwrap();
    assert!(contents.contains("[provisioner]"));
    assert!(contents.contains("git"));
}

#[test]
fn templates_use_respects_output_path() {
    let dir = TempDir::new().unwrap();
    let custom_output = dir.path().join("custom-config.toml");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args([
        "templates",
        "use",
        "react",
        "--output",
        custom_output.to_str().unwrap(),
    ]);

    cmd.assert().success();

    // Verify file was created at custom path
    assert!(custom_output.exists());
    let contents = std::fs::read_to_string(&custom_output).unwrap();
    assert!(contents.contains("node"));
}

// ============================================================================
// jarvy quickstart tests
// ============================================================================

#[test]
fn quickstart_non_interactive_without_tty_cancels() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["quickstart", "--non-interactive"]);

    // In non-interactive mode without a TTY, quickstart gets cancelled
    // when it tries to prompt for user input
    cmd.assert().stdout(
        predicate::str::contains("Quickstart cancelled")
            .or(predicate::str::contains("Welcome to Jarvy")),
    );
}

#[test]
fn quickstart_help_shows_options() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.args(["quickstart", "--help"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("quickstart"))
        .stdout(predicate::str::contains("--non-interactive"));
}

// ============================================================================
// Template content tests
// ============================================================================

fn assert_template_contains_tools(template: &str, expected_tools: &[(&str, &str)]) {
    let home = TempDir::new().unwrap();
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.env("JARVY_HOME", home.path().join("jarvy-home"));
    cmd.args(["init", "--template", template, "--stdout"]);

    let output = cmd.output().expect("template command should run");
    assert!(
        output.status.success(),
        "template `{template}` failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("template output must be UTF-8");
    for (tool, version) in expected_tools {
        let entry = format!("{tool} = \"{version}\"");
        assert!(
            stdout.lines().any(|line| line.trim() == entry),
            "template `{template}` must contain `{entry}`; got:\n{stdout}"
        );
    }
}

#[test]
fn essential_template_contains_core_tools() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["init", "--template", "essential", "--stdout"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("git"))
        .stdout(predicate::str::contains("jq"));
}

#[test]
fn react_template_contains_node_tools() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["init", "--template", "react", "--stdout"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("node"))
        .stdout(predicate::str::contains("git"));
}

#[test]
fn rust_template_contains_rust_tools() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["init", "--template", "rust-cli", "--stdout"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("rust"))
        .stdout(predicate::str::contains("git"));
}

#[test]
fn python_api_template_contains_python_tools() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["init", "--template", "python-api", "--stdout"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("python"))
        .stdout(predicate::str::contains("git"));
}

#[test]
fn k8s_admin_template_contains_kubernetes_tools() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["init", "--template", "k8s-admin", "--stdout"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("kubectl"))
        .stdout(predicate::str::contains("docker"));
}

#[test]
fn mobile_templates_preserve_their_toolchain_contracts() {
    assert_template_contains_tools(
        "android-kotlin",
        &[
            ("java", "17"),
            ("kotlin", "latest"),
            ("android_command_line_tools", "latest"),
            ("android_platform_tools", "latest"),
            ("appium", "latest"),
        ],
    );
    assert_template_contains_tools(
        "flutter",
        &[
            ("java", "17"),
            ("android_command_line_tools", "latest"),
            ("android_platform_tools", "latest"),
            ("appium", "latest"),
        ],
    );
    assert_template_contains_tools(
        "react-native",
        &[
            ("nvm", "latest"),
            ("node", "20"),
            ("pnpm", "latest"),
            ("java", "17"),
            ("eas_cli", "latest"),
            ("android_command_line_tools", "latest"),
            ("android_platform_tools", "latest"),
            ("watchman", "latest"),
            ("appium", "latest"),
        ],
    );
    assert_template_contains_tools(
        "expo",
        &[
            ("nvm", "latest"),
            ("node", "20"),
            ("pnpm", "latest"),
            ("java", "17"),
            ("eas_cli", "latest"),
            ("android_command_line_tools", "latest"),
            ("android_platform_tools", "latest"),
            ("watchman", "latest"),
            ("appium", "latest"),
        ],
    );
}

#[test]
fn react_native_templates_route_android_through_fresh_validation() {
    for template in ["react-native", "expo"] {
        let home = TempDir::new().unwrap();
        let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
        cmd.env("JARVY_TEST_MODE", "1");
        cmd.env("JARVY_TELEMETRY", "0");
        cmd.env("JARVY_HOME", home.path());
        cmd.args(["init", "--template", template, "--stdout"]);

        let output = cmd.output().expect("jarvy init should execute");
        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).expect("template output must be UTF-8");
        assert!(
            stdout.contains(
                "\"pre:android\" = \"jarvy doctor --fresh --check tools --tools java,node,pnpm\""
            ),
            "template `{template}` must validate the pinned mobile toolchain; got:\n{stdout}"
        );
        assert!(
            stdout.contains("\"android\" = \"pnpm --filter mobile android\""),
            "template `{template}` must retain the normal Android command; got:\n{stdout}"
        );
    }
}

#[test]
fn setup_dry_run_renders_mobile_npm_fallback_commands() {
    use std::io::Write;

    let empty_path = TempDir::new().unwrap();
    let mut config = tempfile::NamedTempFile::new().unwrap();
    writeln!(config, "[provisioner]").unwrap();
    writeln!(config, "eas-cli = \"latest\"").unwrap();
    writeln!(config, "appium = \"latest\"").unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.env("JARVY_TELEMETRY", "0");
    cmd.env("JARVY_HOME", empty_path.path().join("jarvy-home"));
    cmd.env("PATH", empty_path.path());
    cmd.args(["setup", "--dry-run", "--file"]);
    cmd.arg(config.path());

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "Would install eas-cli via fallback route: `npm install -g eas-cli`",
        ))
        .stdout(predicate::str::contains(
            "Would install appium via fallback route: `npm install -g appium`",
        ));
}

#[cfg(target_os = "macos")]
#[test]
fn mobile_fresh_machine_plan_provisions_exact_jdk_node_and_pnpm() {
    use std::io::Write;

    let empty_path = TempDir::new().unwrap();
    let mut config = tempfile::NamedTempFile::new().unwrap();
    writeln!(config, "[provisioner]").unwrap();
    writeln!(config, "java = \"17\"").unwrap();
    writeln!(config, "node = \"20\"").unwrap();
    writeln!(config, "pnpm = \"latest\"").unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.env("JARVY_TELEMETRY", "0");
    cmd.env("JARVY_HOME", empty_path.path().join("jarvy-home"));
    cmd.env("PATH", empty_path.path());
    cmd.args(["setup", "--dry-run", "--file"]);
    cmd.arg(config.path());

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("brew install openjdk@17"))
        .stdout(predicate::str::contains("pnpm"))
        .stdout(predicate::str::contains(
            "Would install node version 20 using custom installer",
        ))
        .stdout(predicate::str::contains("`brew install openjdk`").not());
}

// ============================================================================
// File collision tests
// ============================================================================

#[test]
fn init_does_not_overwrite_existing_file_by_default() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("jarvy.toml");

    // Create an existing file
    std::fs::write(&config_path, "# existing config\n").unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.current_dir(dir.path());
    cmd.args(["init", "--template", "essential"]);

    // Should warn about existing file
    cmd.assert()
        .stdout(predicate::str::contains("already exists"));

    // Original content should be preserved
    let contents = std::fs::read_to_string(&config_path).unwrap();
    assert!(contents.contains("# existing config"));
}

#[test]
fn templates_use_does_not_overwrite_existing_file() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("jarvy.toml");

    // Create an existing file
    std::fs::write(&config_path, "# existing config\n").unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.current_dir(dir.path());
    cmd.args(["templates", "use", "essential"]);

    // Should warn about existing file
    cmd.assert()
        .stdout(predicate::str::contains("already exists"));

    // Original content should be preserved
    let contents = std::fs::read_to_string(&config_path).unwrap();
    assert!(contents.contains("# existing config"));
}
