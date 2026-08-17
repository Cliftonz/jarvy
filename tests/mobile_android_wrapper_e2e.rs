//! CI-only end-to-end coverage for the generated mobile Android wrapper.
//!
//! Cargo only discovers this target with the `test-bypass` feature. CI runs
//! `cargo nextest run --all-features`; ordinary local `cargo test` skips it.
//! Every external tool is a fake executable, so this never contacts a local
//! Android device, Gradle installation, package registry, or Homebrew.

#![cfg(all(unix, feature = "test-bypass"))]

use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::Path;
use tempfile::TempDir;

fn write_executable(path: &Path, body: &str) {
    fs::write(path, body).expect("fake executable should be written");
    let mut permissions = fs::metadata(path)
        .expect("fake executable should exist")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("fake executable should be executable");
}

fn write_version_tool(bin: &Path, name: &str, version_output: &str) {
    write_executable(
        &bin.join(name),
        &format!("#!/bin/sh\nprintf '%s\\n' '{version_output}'\n"),
    );
}

fn wrapper_command(
    jarvy: &Path,
    project: &Path,
    bin: &Path,
    jarvy_home: &Path,
    android_log: &Path,
) -> Command {
    let mut command = Command::new(jarvy);
    command.current_dir(project);
    command.env("PATH", bin);
    command.env("FAKE_BIN", bin);
    command.env("ANDROID_LOG", android_log);
    command.env("JARVY_HOME", jarvy_home);
    command.env("JARVY_TEST_MODE", "1");
    command.env("JARVY_TELEMETRY", "0");
    command.args(["run", "android", "--file", "jarvy.toml"]);
    command
}

#[test]
fn android_wrapper_blocks_invalid_tools_then_runs_exact_pnpm_command() {
    let workspace = TempDir::new().expect("temporary workspace should be created");
    let project = workspace.path().join("project");
    let bin = workspace.path().join("bin");
    let jarvy_home = workspace.path().join("jarvy-home");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&bin).unwrap();

    let source_jarvy = cargo_bin!("jarvy");
    let path_jarvy = bin.join("jarvy");
    fs::copy(source_jarvy, &path_jarvy).expect("jarvy should be available to the pre-hook");
    let mut jarvy_permissions = fs::metadata(&path_jarvy).unwrap().permissions();
    jarvy_permissions.set_mode(0o755);
    fs::set_permissions(&path_jarvy, jarvy_permissions).unwrap();
    symlink("/bin/sh", bin.join("sh")).expect("test shell should be available on isolated PATH");

    write_executable(
        &bin.join("which"),
        "#!/bin/sh\ncandidate=\"$FAKE_BIN/$1\"\nif [ -x \"$candidate\" ]; then\n  printf '%s\\n' \"$candidate\"\n  exit 0\nfi\nexit 1\n",
    );
    write_version_tool(&bin, "java", "openjdk 26 2026-03-17");
    write_version_tool(&bin, "node", "v20.12.2");

    let android_log = workspace.path().join("android-command.log");
    write_executable(
        &bin.join("pnpm"),
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ] || [ \"$1\" = \"-v\" ] || [ \"$1\" = \"-V\" ]; then\n  printf '%s\\n' '9.0.0'\n  exit 0\nfi\nprintf '%s\\n' \"$*\" >> \"$ANDROID_LOG\"\n",
    );

    let mut init = Command::new(source_jarvy);
    init.env("JARVY_TEST_MODE", "1");
    init.env("JARVY_TELEMETRY", "0");
    init.env("JARVY_HOME", &jarvy_home);
    init.args(["init", "--template", "react-native", "--stdout"]);
    let generated = init.output().expect("template generation should execute");
    assert!(generated.status.success());
    fs::write(project.join("jarvy.toml"), generated.stdout)
        .expect("generated config should be written");

    wrapper_command(&path_jarvy, &project, &bin, &jarvy_home, &android_log)
        .assert()
        .failure();
    assert!(
        !android_log.exists(),
        "Java 26 must not satisfy the generated Java 17 requirement"
    );

    write_version_tool(&bin, "java", "openjdk 17.0.12 2024-07-16");
    write_version_tool(&bin, "node", "v22.4.0");
    wrapper_command(&path_jarvy, &project, &bin, &jarvy_home, &android_log)
        .assert()
        .failure();
    assert!(
        !android_log.exists(),
        "Node 22 must not satisfy the generated Node 20 requirement"
    );

    write_version_tool(&bin, "node", "v20.12.2");
    fs::remove_file(bin.join("pnpm")).expect("pnpm should be removed for the missing-tool case");
    wrapper_command(&path_jarvy, &project, &bin, &jarvy_home, &android_log)
        .assert()
        .failure();
    assert!(
        !android_log.exists(),
        "a missing pnpm must block the Android command"
    );

    write_executable(
        &bin.join("pnpm"),
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ] || [ \"$1\" = \"-v\" ] || [ \"$1\" = \"-V\" ]; then\n  printf '%s\\n' '9.0.0'\n  exit 0\nfi\nprintf '%s\\n' \"$*\" >> \"$ANDROID_LOG\"\n",
    );
    wrapper_command(&path_jarvy, &project, &bin, &jarvy_home, &android_log)
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(&android_log).expect("successful wrapper must execute pnpm"),
        "--filter mobile android\n"
    );
}
