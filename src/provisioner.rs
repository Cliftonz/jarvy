use std::env;
use std::str;

use crate::telemetry;
use crate::tools::common::run_capture;

// The pinned-installer constants and helpers live in
// `crate::tools::brew::definition` so they can be reused by the tool-registry
// brew installer path (which is in lib code and cannot import from `main`).
use crate::tools::brew::{
    HOMEBREW_INSTALLER_COMMIT, brew_auto_install_allowed, pinned_homebrew_installer_command,
};

pub fn install_homebrew() {
    // Macos Only
    let Some(test_brew_cmd) =
        run_capture("brew", &["--version"], "macos_setup", "Failed to run brew")
    else {
        return;
    };

    if !test_brew_cmd.status.success() {
        if !brew_auto_install_allowed() {
            eprintln!(
                "Refusing to auto-install Homebrew in CI. \
                 Pre-install brew in your runner image or set up brew via your CI provisioner."
            );
            telemetry::tool_failed("homebrew", "latest", "refused-in-ci");
            return;
        }

        println!("Installing Homebrew (pinned to commit {HOMEBREW_INSTALLER_COMMIT})");
        let start = telemetry::now();

        let verify_then_exec = pinned_homebrew_installer_command();

        if run_capture(
            "sh",
            &["-c", &verify_then_exec],
            "macos_setup",
            "Failed to execute pinned Homebrew installer",
        )
        .is_none()
        {
            return;
        }

        let home = env::var("HOME").unwrap_or_else(|_| "~".to_string());
        let zprofile = format!("{}/.zprofile", home);
        let apple_chip_brew_bin = "/opt/homebrew/bin";
        let brew_bin = "/usr/local/bin";
        let entry = format!(
            "export PATH={}:{}:${}",
            brew_bin,
            apple_chip_brew_bin,
            env::var("PATH").unwrap_or_default()
        );

        let param_to_cmd = format!("grep -R {} {}", entry, zprofile);

        if run_capture(
            "sh",
            &["-c", &param_to_cmd],
            "macos_setup",
            "Failed to execute grep command",
        )
        .is_none()
        {
            return;
        }

        let Some(after_install_test_cmd) = run_capture(
            "brew",
            &["--version"],
            "macos_setup",
            "Failed to execute brew version check",
        ) else {
            return;
        };

        if !after_install_test_cmd.status.success() {
            eprintln!("Error: Homebrew");
            telemetry::tool_failed("homebrew", "latest", "installation failed");
        } else {
            println!("Successfully installed Homebrew");
            telemetry::tool_installed("homebrew", "latest", "shell", start.elapsed());
        }
    } else {
        println!("Homebrew is already installed");
    }
}

pub fn install_docker() {
    let Some(check_homebrew_output) = run_capture(
        "brew",
        &["--version"],
        "macos_setup",
        "Failed to execute brew version check",
    ) else {
        return;
    };

    // If brew not found or any other problem occurred
    if check_homebrew_output.status.success() {
        let Some(test_docker_output) = run_capture(
            "docker",
            &["--version"],
            "macos_setup",
            "Failed to execute docker version check",
        ) else {
            return;
        };

        // If docker not found or any other problem occurred
        if !test_docker_output.status.success() {
            println!("Installing Docker");
            let start = telemetry::now();
            let Some(brew_install_output) = run_capture(
                "brew",
                &["install", "docker"],
                "macos_setup",
                "Failed to execute brew install docker",
            ) else {
                return;
            };

            // If Docker installed successfully
            if brew_install_output.status.success() {
                // Test Docker installation
                let Some(after_install_test_output) = run_capture(
                    "docker",
                    &["--version"],
                    "macos_setup",
                    "Failed to execute docker version check",
                ) else {
                    return;
                };

                // If Docker now runs properly
                if after_install_test_output.status.success() {
                    println!("Successfully installed Docker");
                    telemetry::tool_installed("docker", "latest", "brew", start.elapsed());
                } else {
                    println!("Error: Docker");
                    telemetry::tool_failed("docker", "latest", "post-install test failed");
                }
            } else {
                telemetry::tool_failed("docker", "latest", "brew install failed");
            }
        } else {
            println!("Docker is already installed");
        }
    } else {
        println!("Skipping Docker installation as Homebrew is not found");
    }
}

/// Start docker infrastructure using docker-compose.
///
/// # Arguments
/// * `compose_file` - Path to the docker-compose file. Defaults to `./docker/docker-compose.yml`.
pub fn start_docker_infra_with_config(compose_file: Option<&str>) {
    let compose_path = compose_file.unwrap_or("./docker/docker-compose.yml");
    let start = telemetry::now();
    let Some(docker_compose_output) = run_capture(
        "docker-compose",
        &["-f", compose_path, "up", "-d"],
        "docker_infra",
        "Failed to execute docker-compose command",
    ) else {
        return;
    };

    if docker_compose_output.status.success() {
        println!("Successfully started Docker Infrastructure");
        telemetry::service_operation("docker-compose", "up", true);
    } else {
        let err = String::from_utf8_lossy(&docker_compose_output.stderr).to_string();
        eprintln!(
            "An error occurred: \n\t {}. \nPlease run this from the root of your repository.",
            err
        );
        telemetry::service_operation("docker-compose", "up", false);
    }
    // Track duration even if unused for now
    let _ = start.elapsed();
}

// Minimal stubs to satisfy references from setup.rs during tests
#[allow(dead_code)] // Test stub
pub fn install_nvm_mac() {
    // no-op in test context
}

#[allow(dead_code)] // Test stub
pub fn install_pnpm() {
    // no-op in test context
}

pub fn check_and_install_git(_platform: &str) {
    // no-op in test context
}
