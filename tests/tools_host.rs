// Host environment smoke tests for macOS (brew) and Windows (winget).
// These are intentionally minimal and side‑effect free. They exist primarily
// to provide a stable test target invoked by CI workflows on native hosts.
//
// The CI workflow sets JARVY_BIN to the path of the built jarvy binary.
// We simply invoke `--help` to ensure the binary runs on the host.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn jarvy_bin_path() -> PathBuf {
    if let Ok(p) = env::var("JARVY_BIN") {
        return PathBuf::from(p);
    }
    // Fallback for local runs: assume release binary in target/release
    let mut p = PathBuf::from("target/release");
    #[cfg(windows)]
    {
        p.push("jarvy.exe");
    }
    #[cfg(not(windows))]
    {
        p.push("jarvy");
    }
    p
}

#[test]
fn jarvy_help_runs() {
    let bin = jarvy_bin_path();
    let output = Command::new(&bin)
        .arg("--help")
        .output()
        .expect("failed to execute jarvy binary");
    assert!(
        output.status.success(),
        "jarvy --help exited with non-zero status"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn host_has_brew() {
    // The hosted macOS runners include Homebrew preinstalled.
    // Verify it is callable. This is a cheap, deterministic check.
    let status = Command::new("brew")
        .arg("--version")
        .status()
        .expect("failed to execute brew");
    assert!(status.success(), "brew --version did not succeed");
}

#[cfg(target_os = "windows")]
#[test]
fn host_has_winget() {
    // The hosted Windows runners include winget and we updated sources in the workflow.
    let status = Command::new("winget")
        .arg("--info")
        .status()
        .expect("failed to execute winget");
    assert!(status.success(), "winget --info did not succeed");
}
