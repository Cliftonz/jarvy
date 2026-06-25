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

/// Outcome of the OMZ-detection step in `check_zsh`. Extracting this
/// from the IO-heavy `check_zsh` makes the regression "prompt fired
/// even though `~/.oh-my-zsh` was already present" table-testable.
#[derive(Debug, PartialEq, Eq)]
enum OmzAction {
    /// `~/.oh-my-zsh` exists — skip the prompt entirely.
    AlreadyInstalled,
    /// Prompt the user; on "Yes", install via the upstream installer.
    Install,
    /// User said "No" or the prompt errored.
    Decline,
    /// `dirs::home_dir()` returned `None` — short-circuit silently.
    NoHome,
}

/// Pure decision function for the OMZ install path. The `prompt`
/// closure returns `Some("Yes" | "No")` to mirror what `inquire`
/// produces, or `None` to mean "prompt errored" (treated as Decline).
/// `home` of `None` short-circuits before the prompt is even
/// reached — verified by passing a closure that panics in tests.
fn decide_omz_action(home: Option<&Path>, prompt: impl FnOnce() -> Option<String>) -> OmzAction {
    let Some(home) = home else {
        return OmzAction::NoHome;
    };
    if home.join(".oh-my-zsh").exists() {
        return OmzAction::AlreadyInstalled;
    }
    match prompt().as_deref() {
        Some("Yes") => OmzAction::Install,
        _ => OmzAction::Decline,
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

    let home = dirs::home_dir();
    let action = decide_omz_action(home.as_deref(), || {
        Select::new("Do you want to install Oh My Zsh?", vec!["Yes", "No"])
            .prompt()
            .ok()
            .map(|s| s.to_string())
    });

    match action {
        OmzAction::AlreadyInstalled => {
            // home is Some(_) because AlreadyInstalled requires it; safe.
            let ohmyzsh_dir = home.unwrap().join(".oh-my-zsh");
            let display = ohmyzsh_dir.to_string_lossy();
            println!("Oh My Zsh! is already installed.");
            telemetry::tool_already_installed(
                "oh-my-zsh",
                &display,
                "path_exists",
                "check_zsh",
                false,
            );
        }
        OmzAction::Install => {
            let ohmyzsh_dir = home.unwrap().join(".oh-my-zsh");
            if let Err(e) = Command::new("sh")
                .arg("-c")
                .arg("$(curl -fsSL https://raw.github.com/ohmyzsh/ohmyzsh/master/tools/install.sh)")
                .status()
            {
                eprintln!("Failed to install Oh My Zsh: {e}");
                return;
            }
            if !ohmyzsh_dir.exists() {
                eprintln!(
                    "Oh My Zsh installer exited but {} was not created. \
                     Try the manual installer: https://ohmyz.sh",
                    ohmyzsh_dir.display()
                );
            }
        }
        OmzAction::Decline | OmzAction::NoHome => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Closure that panics if invoked — proves the AlreadyInstalled
    /// path short-circuits BEFORE the prompt. This is the regression
    /// guard for the "OMZ re-prompt on every setup" bug.
    fn never_prompt() -> Option<String> {
        panic!("prompt must not be called on the AlreadyInstalled path")
    }

    #[test]
    fn decide_omz_action_already_installed_skips_prompt() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".oh-my-zsh")).unwrap();
        let action = decide_omz_action(Some(dir.path()), never_prompt);
        assert_eq!(action, OmzAction::AlreadyInstalled);
    }

    #[test]
    fn decide_omz_action_no_dir_prompts_yes_returns_install() {
        let dir = tempdir().unwrap();
        let action = decide_omz_action(Some(dir.path()), || Some("Yes".to_string()));
        assert_eq!(action, OmzAction::Install);
    }

    #[test]
    fn decide_omz_action_no_dir_prompts_no_returns_decline() {
        let dir = tempdir().unwrap();
        let action = decide_omz_action(Some(dir.path()), || Some("No".to_string()));
        assert_eq!(action, OmzAction::Decline);
    }

    #[test]
    fn decide_omz_action_prompt_error_returns_decline() {
        let dir = tempdir().unwrap();
        let action = decide_omz_action(Some(dir.path()), || None);
        assert_eq!(action, OmzAction::Decline);
    }

    #[test]
    fn decide_omz_action_no_home_short_circuits_before_prompt() {
        let action = decide_omz_action(None, never_prompt);
        assert_eq!(action, OmzAction::NoHome);
    }
}
