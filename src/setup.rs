use std::path::Path;
use std::process::Command;
use std::{env, str};

use inquire::Select;

use crate::os_setup::set_up_os;
use crate::outputs::{error_message, installing_dependency, success_message};
use crate::provisioner::{
    check_and_install_git, install_docker, install_homebrew, start_docker_infra_with_config,
};
use crate::telemetry;

// Main function
pub fn setup() {
    const PLATFORM: &str = env::consts::OS;
    let start = telemetry::now();

    println!("Detecting Platform is: {}\n", PLATFORM);

    println!("Setting up defaults\n");
    set_up_os(PLATFORM);

    println!("\nInstalling Required Tools for {}\n", PLATFORM);

    check_hard_dependencies(PLATFORM);
    check_and_install_git(PLATFORM);
    install_docker();

    match PLATFORM {
        "macos" => {
            // install homebrew
            install_homebrew();
        }
        "linux" => {}
        "windows" => {}
        _ => {}
    }

    start_docker_infra_with_config(None);

    // Emit setup_complete with duration
    let summary = telemetry::SetupSummary {
        tools_requested: 0, // Legacy setup - minimal tracking
        tools_installed: 0,
        tools_skipped: 0,
        tools_failed: 0,
        hooks_run: 0,
        duration: start.elapsed(),
    };
    telemetry::setup_completed(&summary);
}

fn check_hard_dependencies(platform: &str) {
    // `platform` is `env::consts::OS` — lowercase. Same case-mismatch
    // bug as refresh_shell (was "macOS"); never fired on actual
    // macOS hosts. Hard-dep check is now actually reachable.
    match platform {
        "macos" => {
            let Some(output) = crate::tools::common::run_capture(
                "brew",
                &["--version"],
                "hard_dep_check",
                "Failed to run Homebrew check",
            ) else {
                return;
            };

            let brew_check = str::from_utf8(&output.stdout).unwrap_or("");

            if brew_check.is_empty() || output.status.code() != Some(0) {
                error_message("Homebrew");
                println!("⛔️ Homebrew is a hard dependency for this tool");

                installing_dependency("Homebrew");
                let Some(output) = crate::tools::common::run_capture(
                    "/bin/bash",
                    &[
                        "-c",
                        r#""$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)""#,
                    ],
                    "hard_dep_check",
                    "Failed to execute Homebrew install command",
                ) else {
                    return;
                };

                println!("{}", String::from_utf8_lossy(&output.stdout));
                success_message("Homebrew")
            }

            check_zsh();
        }
        "windows" => {}
        _ => {}
    }
}

fn check_zsh() {
    // Check if zsh is installed
    let Some(output) = crate::tools::common::run_capture(
        "zsh",
        &["--version"],
        "hard_dep_check",
        "Failed to check zsh",
    ) else {
        return;
    };

    // If zsh is not installed, don't go further.
    if output.status.code() != Some(0) {
        return;
    }

    let Some(home) = dirs::home_dir() else {
        return;
    };
    let ohmyzsh_dir = format!("{}/.oh-my-zsh", home.display());

    // Skip prompt entirely when Oh My Zsh already installed —
    // re-prompting on every `jarvy setup` is pure noise.
    if Path::new(&ohmyzsh_dir).exists() {
        println!("Oh My Zsh! is already installed.");
        telemetry::tool_already_installed(
            "oh-my-zsh",
            &ohmyzsh_dir,
            "path_exists",
            "check_zsh",
            false,
        );
        return;
    }

    let user_choice = Select::new("Do you want to install Oh My Zsh?", vec!["Yes", "No"]).prompt();

    let Ok(response) = user_choice else {
        return;
    };

    if response != "Yes" {
        return;
    }

    if let Err(e) = Command::new("sh")
        .arg("-c")
        .arg("$(curl -fsSL https://raw.github.com/ohmyzsh/ohmyzsh/master/tools/install.sh)")
        .status()
    {
        eprintln!("Failed to install Oh My Zsh: {e}");
        return;
    }

    if !Path::new(&ohmyzsh_dir).exists() {
        println!("Error: Oh My Zsh!");
    }
}
