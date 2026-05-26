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
    // Resolve the host-correct path to the binary built by Cargo for this test run
    assert_cmd::cargo::cargo_bin!("jarvy").to_path_buf()
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

// End-to-end real-install smoke. Catches regressions where the
// macOS setup pipeline aborts before reaching the tool-install
// loop (e.g. an earlier bug where `refresh_shell()` sourced
// `~/.zprofile` via `/bin/sh` and exit(1)'d on any zsh-only syntax
// in the user's dotfiles, or `exec`d the user's shell mid-flow and
// replaced the jarvy process so tools never installed). `jq` is
// idempotent under brew and small enough to keep the runner cost
// low; the assertion is that `jarvy setup --file` returns 0 — i.e.
// the pipeline ran to completion.
#[cfg(target_os = "macos")]
#[test]
fn macos_setup_install_completes() {
    let bin = jarvy_bin_path();

    let tmp = env::temp_dir().join("jarvy_macos_setup_install_completes.toml");
    let mut f = std::fs::File::create(&tmp).expect("create temp toml");
    writeln!(
        f,
        "[privileges]\nuse_sudo = false\n\n[provisioner]\njq = \"latest\"\n"
    )
    .expect("write temp toml");
    drop(f);

    let output = Command::new(&bin)
        .args(["setup", "--file"])
        .arg(&tmp)
        .env("JARVY_TEST_MODE", "1")
        .env("JARVY_TELEMETRY", "0")
        .output()
        .expect("failed to execute jarvy setup");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "jarvy setup --file exited non-zero\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
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
