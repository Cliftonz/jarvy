// Single-tool installation test for jq.
// Intentionally mirrors the environment guard from the parallel install test
// and is primarily expected to run on Linux CI runners where a package manager
// is available and sudo can run non-interactively.

#[test]
fn install_jq_only() {
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

    // Attempt to install/ensure jq
    match jarvy::tools::add("jq", "", &jarvy::tools::common::InstallContext::none()) {
        Ok(()) => {}
        Err(e) => panic!("jq install failed: {:?}", e),
    }
}
