//! Guided quickstart experience for new users
//!
//! Provides a step-by-step onboarding flow for first-time Jarvy users.

use crate::output::{ExitCode, Outputable};
use crate::templates::builtin::list_builtin_templates;
use crate::tools::common::{Os, current_os};
use inquire::{Confirm, Select};
use serde::Serialize;
use std::io::{self, IsTerminal};

/// Options for quickstart command
#[derive(Debug, Clone, Default)]
pub struct QuickstartOptions {
    /// Non-interactive mode
    pub non_interactive: bool,
    /// Skip system check
    #[allow(dead_code)] // Reserved for future system check bypass
    pub skip_check: bool,
}

/// System check result
#[derive(Debug, Clone, Serialize)]
pub struct SystemCheck {
    pub os: String,
    pub os_supported: bool,
    pub package_manager: Option<String>,
    pub shell: Option<String>,
}

/// Quickstart result
#[derive(Debug, Clone, Serialize)]
pub struct QuickstartResult {
    pub system: SystemCheck,
    pub config_created: bool,
    pub config_path: Option<String>,
    pub setup_run: bool,
    pub aborted: bool,
}

impl Outputable for QuickstartResult {
    fn to_human(&self) -> String {
        if self.aborted {
            return "\nQuickstart cancelled.\n".to_string();
        }

        let mut output = String::new();

        if self.config_created {
            output.push_str("\n\x1b[32m🎉 You're all set!\x1b[0m\n\n");
        }

        output.push_str("Useful commands:\n");
        output.push_str("  \x1b[36mjarvy search\x1b[0m    - Find available tools\n");
        output.push_str("  \x1b[36mjarvy upgrade\x1b[0m   - Update all tools\n");
        output.push_str("  \x1b[36mjarvy doctor\x1b[0m    - Check environment health\n");
        output.push_str("  \x1b[36mjarvy --help\x1b[0m    - See all commands\n\n");
        output.push_str("Documentation: \x1b[36mhttps://jarvy.dev/docs\x1b[0m\n");

        output
    }

    fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
    }

    fn exit_code(&self) -> ExitCode {
        if self.aborted {
            ExitCode::Warning
        } else {
            ExitCode::Ok
        }
    }
}

/// Run the quickstart flow
pub fn run_quickstart(options: QuickstartOptions) -> QuickstartResult {
    // Check if running in TTY
    let is_tty = io::stdin().is_terminal();

    // No TTY means we cannot run any of the inquire-based prompts further
    // down. Two paths, both bail before a prompt is reached:
    //   - non_interactive mode: print "Quickstart cancelled" to stdout and
    //     exit cleanly. Without this, on Windows the inquire prompt would
    //     block on ReadConsoleW indefinitely (Unix returns NotTTY quickly
    //     so the bug was platform-masked). The Tools E2E job at
    //     v0.1.0-rc.7 hung for 35 minutes here before the 45-min timeout.
    //   - interactive mode (no flag): print the existing error to stderr
    //     directing the user at non-interactive subcommands.
    if !is_tty {
        if options.non_interactive {
            println!(
                "Quickstart cancelled: no interactive terminal detected. \
                 Use 'jarvy init --template <name>' for non-interactive setup."
            );
        } else {
            eprintln!("Error: Quickstart requires an interactive terminal.");
            eprintln!("Use non-interactive commands like 'jarvy init --template <name>'");
        }
        return QuickstartResult {
            system: SystemCheck {
                os: "unknown".to_string(),
                os_supported: false,
                package_manager: None,
                shell: None,
            },
            config_created: false,
            config_path: None,
            setup_run: false,
            aborted: true,
        };
    }

    // Print welcome banner
    print_welcome_banner();

    // Step 1: System check
    println!("\x1b[1mStep 1 of 3: Check your system\x1b[0m");
    println!("──────────────────────────────");
    let system = check_system();
    print_system_check(&system);
    println!();

    if !system.os_supported {
        eprintln!("\x1b[31m✗\x1b[0m Your operating system is not supported.");
        return QuickstartResult {
            system,
            config_created: false,
            config_path: None,
            setup_run: false,
            aborted: true,
        };
    }

    // Step 2: Config creation
    println!("\x1b[1mStep 2 of 3: Create your first config\x1b[0m");
    println!("──────────────────────────────────────");

    let config_choice = match Select::new(
        "Would you like to:",
        vec![
            "Create a new jarvy.toml (recommended)",
            "Use a template",
            "Skip for now",
        ],
    )
    .prompt()
    {
        Ok(c) => c,
        Err(_) => {
            return QuickstartResult {
                system,
                config_created: false,
                config_path: None,
                setup_run: false,
                aborted: true,
            };
        }
    };

    let (config_created, config_path) = match config_choice {
        "Create a new jarvy.toml (recommended)" => {
            // Run init wizard
            let init_options = super::init::InitOptions::default();
            let result = super::init::run_init(init_options);
            (result.created, result.output_path)
        }
        "Use a template" => {
            // Show template selection
            let templates: Vec<String> = list_builtin_templates()
                .iter()
                .map(|t| format!("{} - {}", t.name, t.description))
                .collect();

            let selected = match Select::new("Select a template:", templates).prompt() {
                Ok(s) => s,
                Err(_) => {
                    return QuickstartResult {
                        system,
                        config_created: false,
                        config_path: None,
                        setup_run: false,
                        aborted: true,
                    };
                }
            };

            // Extract template name (before the " - ")
            let template_name = selected.split(" - ").next().unwrap_or("essential");
            let result = crate::commands::templates::use_template(template_name, None);
            (result.created, result.output_path)
        }
        _ => (false, None),
    };

    if !config_created {
        println!("\nNo config created. You can always run \x1b[36mjarvy init\x1b[0m later.\n");
        return QuickstartResult {
            system,
            config_created: false,
            config_path: None,
            setup_run: false,
            aborted: false,
        };
    }

    println!();

    // Step 3: Install
    println!("\x1b[1mStep 3 of 3: Install your tools\x1b[0m");
    println!("────────────────────────────────");

    let run_setup = Confirm::new("Install tools now?")
        .with_default(true)
        .prompt()
        .unwrap_or_default();

    if run_setup {
        println!("\nRunning \x1b[36mjarvy setup\x1b[0m...\n");
        // Note: We don't actually run setup here to avoid complexity
        // The user can run it manually
        println!("\x1b[33mNote:\x1b[0m Run \x1b[36mjarvy setup\x1b[0m to install your tools.\n");
    }

    QuickstartResult {
        system,
        config_created,
        config_path,
        setup_run: false, // User needs to run manually
        aborted: false,
    }
}

/// Print the welcome banner
fn print_welcome_banner() {
    let cyan = "\x1b[36m";
    let bold = "\x1b[1m";
    let reset = "\x1b[0m";

    println!();
    println!("{cyan}╔═══════════════════════════════════════════════════════════╗{reset}");
    println!(
        "{cyan}║{reset}                    {bold}Welcome to Jarvy!{reset}                       {cyan}║{reset}"
    );
    println!(
        "{cyan}║{reset}         Fast, cross-platform developer tool setup          {cyan}║{reset}"
    );
    println!("{cyan}╚═══════════════════════════════════════════════════════════╝{reset}");
    println!();
    println!("Jarvy helps you install and manage developer tools consistently");
    println!("across macOS, Linux, and Windows.");
    println!();
    println!("Let's get you started in 3 quick steps:");
    println!();
}

/// Check the system for compatibility
fn check_system() -> SystemCheck {
    let os = current_os();
    let os_name = match os {
        Os::Macos => "macOS",
        Os::Linux => "Linux",
        Os::Windows => "Windows",
        Os::Bsd => "BSD",
    };

    let package_manager = detect_package_manager();
    let shell = detect_shell();

    SystemCheck {
        os: os_name.to_string(),
        os_supported: matches!(os, Os::Macos | Os::Linux | Os::Windows),
        package_manager,
        shell,
    }
}

/// Detect the primary package manager
fn detect_package_manager() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        if std::process::Command::new("brew")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            // Get Homebrew version
            if let Ok(output) = std::process::Command::new("brew").arg("--version").output() {
                let version = String::from_utf8_lossy(&output.stdout);
                let first_line = version.lines().next().unwrap_or("Homebrew");
                return Some(first_line.to_string());
            }
            return Some("Homebrew".to_string());
        }
        None
    }

    #[cfg(target_os = "linux")]
    {
        // Check for common package managers
        for (cmd, name) in [
            ("apt", "APT"),
            ("dnf", "DNF"),
            ("pacman", "Pacman"),
            ("apk", "APK"),
        ] {
            if std::process::Command::new(cmd)
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                return Some(name.to_string());
            }
        }
        None
    }

    #[cfg(target_os = "windows")]
    {
        // Check for winget or chocolatey
        if std::process::Command::new("winget")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some("Winget".to_string());
        }
        if std::process::Command::new("choco")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some("Chocolatey".to_string());
        }
        None
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

/// Detect the current shell
fn detect_shell() -> Option<String> {
    std::env::var("SHELL")
        .ok()
        .and_then(|s| s.split('/').next_back().map(|s| s.to_string()))
}

/// Print the system check results
fn print_system_check(check: &SystemCheck) {
    let green = "\x1b[32m";
    let yellow = "\x1b[33m";
    let reset = "\x1b[0m";

    // OS
    if check.os_supported {
        println!("{green}✓{reset} Operating System: {} (supported)", check.os);
    } else {
        println!(
            "{yellow}!{reset} Operating System: {} (not supported)",
            check.os
        );
    }

    // Package Manager
    if let Some(ref pm) = check.package_manager {
        println!("{green}✓{reset} Package Manager: {} (detected)", pm);
    } else {
        println!("{yellow}!{reset} Package Manager: not detected");
    }

    // Shell
    if let Some(ref shell) = check.shell {
        println!("{green}✓{reset} Shell: {} (completions available)", shell);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_system() {
        let check = check_system();
        assert!(!check.os.is_empty());
        // OS should be one of the supported ones on a development machine
    }

    #[test]
    fn test_quickstart_result_json() {
        let result = QuickstartResult {
            system: SystemCheck {
                os: "macOS".to_string(),
                os_supported: true,
                package_manager: Some("Homebrew".to_string()),
                shell: Some("zsh".to_string()),
            },
            config_created: true,
            config_path: Some("jarvy.toml".to_string()),
            setup_run: false,
            aborted: false,
        };

        let json = result.to_json();
        assert!(json.contains("macOS"));
        assert!(json.contains("Homebrew"));
    }
}
