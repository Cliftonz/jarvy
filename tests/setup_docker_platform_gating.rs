//! Regression guard for the Homebrew-on-non-macOS bug: `setup()` used to
//! call `install_docker()` unconditionally for every OS instead of only
//! inside the macOS arm of the platform match, which made `jarvy setup`
//! shell out to `brew --version` on Linux/Windows and print a spurious
//! "Failed to execute brew version check" line. This test runs the real
//! compiled binary on a non-macOS platform and asserts that line never
//! appears.

#![cfg(not(target_os = "macos"))]

use std::io::Write;
use std::process::Command;
use tempfile::{NamedTempFile, TempDir};

fn jarvy(home: &TempDir) -> Command {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin!("jarvy"));
    c.env("JARVY_TEST_MODE", "1")
        .env("JARVY_SANDBOX", "0")
        .env("JARVY_TELEMETRY", "0")
        .env("JARVY_FAST_TEST", "1")
        .env("JARVY_MCP_REGISTER", "0")
        .env("JARVY_NO_PERSONAL_CONFIG", "1")
        .env("HOME", home.path())
        .env("JARVY_HOME", home.path())
        .env_remove("JARVY_CHATTER")
        .env_remove("CLAUDECODE");
    c
}

fn write_config() -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(
        f,
        r#"[privileges]
use_sudo = false

[provisioner]
"#
    )
    .unwrap();
    f
}

#[test]
fn setup_never_attempts_brew_on_non_macos() {
    let home = TempDir::new().unwrap();
    let cfg = write_config();
    let mut cmd = jarvy(&home);
    cmd.args(["setup", "--file"]).arg(cfg.path());
    let output = cmd.output().expect("failed to spawn jarvy");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    assert!(
        !stderr.contains("Failed to execute brew version check"),
        "jarvy setup must never invoke brew on non-macOS platforms; got stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("Failed to run brew"),
        "jarvy setup must never invoke brew on non-macOS platforms; got stderr:\n{stderr}"
    );
}
