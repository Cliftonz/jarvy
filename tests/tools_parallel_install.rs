// Parallel installation test for a fixed set of tools.
// Intentionally limited to Linux runners where package managers are available in CI.
// The test attempts to ensure/install git, docker, and jq concurrently.
// If any tool fails to install, the test fails.
//
// Rationale:
// - On macOS and Windows, GUI/cask installers (e.g., Docker Desktop) or winget prompts
//   may cause flakiness in CI, so we scope this test to Linux only.

#![cfg(target_os = "linux")]

#[test]
fn install_git_docker_jq_in_parallel() {
    use std::thread;

    // Make sure the registry has the built-in tools
    jarvy::tools::register_all();

    // Environment guard: require sudo or root for package installs
    let is_root = std::process::Command::new("id")
        .arg("-u")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
        .unwrap_or(false);
    if !is_root {
        let has_sudo = std::process::Command::new("sudo")
            .arg("--version")
            .output()
            .is_ok();
        if !has_sudo {
            eprintln!(
                "Skipping test: sudo not available and not running as root; cannot install packages"
            );
            return;
        }
        // Ensure sudo works non-interactively (no password prompt)
        let sudo_works = std::process::Command::new("sudo")
            .arg("-n")
            .arg("true")
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !sudo_works {
            eprintln!(
                "Skipping test: sudo requires a password or TTY; cannot install packages non-interactively"
            );
            return;
        }
    }

    let tool_names = ["git", "docker", "jq"];

    // Spawn a thread per tool to invoke the registry add/ensure path
    let handles: Vec<_> = tool_names
        .into_iter()
        .map(|name| {
            let n = name.to_string();
            thread::spawn(move || -> Result<(), String> {
                match jarvy::tools::add(&n, "") {
                    Ok(()) => Ok(()),
                    Err(e) => Err(format!("{} install failed: {:?}", n, e)),
                }
            })
        })
        .collect();

    // Join and collect any errors
    let mut errors = Vec::new();
    for h in handles {
        match h.join() {
            Ok(Ok(())) => {}
            Ok(Err(msg)) => errors.push(msg),
            Err(panic) => {
                if let Some(s) = panic.downcast_ref::<&str>() {
                    errors.push(format!("thread panicked: {}", s));
                } else if let Some(s) = panic.downcast_ref::<String>() {
                    errors.push(format!("thread panicked: {}", s));
                } else {
                    errors.push("thread panicked with unknown payload".to_string());
                }
            }
        }
    }

    assert!(
        errors.is_empty(),
        "Parallel installs had failures:\n{}",
        errors.join("\n")
    );
}
