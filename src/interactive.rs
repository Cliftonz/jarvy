//! Interactive menu and user prompts
//!
//! This module handles the interactive menu that appears when Jarvy is run
//! without a subcommand, including first-run welcome experience.

use inquire::{InquireError, Select};

use crate::commands;
use crate::onboarding::{WelcomeBannerConfig, is_first_run, mark_initialized, show_welcome_banner};
use crate::output::Outputable;
use crate::setup::setup;

/// Display the interactive menu for users who run jarvy without a subcommand
pub fn user_select() {
    // Test mode: avoid interactive prompts and side-effects
    if std::env::var("JARVY_TEST_MODE").as_deref() == Ok("1") {
        println!("TEST: user_select invoked");
        return;
    }

    // Check if this is the first run
    if is_first_run() {
        // Show welcome banner for first-time users
        let use_colors = std::io::IsTerminal::is_terminal(&std::io::stdout());
        show_welcome_banner(&WelcomeBannerConfig {
            enabled: true,
            use_colors,
        });

        // Offer first-run options
        let options = vec![
            "Run quickstart (guided setup)",
            "Create a config (jarvy init)",
            "Browse templates",
            "Skip for now",
        ];

        let selection: Result<&str, InquireError> =
            Select::new("How would you like to get started?", options).prompt();

        match selection {
            Ok(choice) => match choice {
                "Run quickstart (guided setup)" => {
                    let options = commands::quickstart::QuickstartOptions::default();
                    let result = commands::quickstart::run_quickstart(options);
                    println!("{}", result.to_human());
                    // Mark as initialized after quickstart
                    let _ = mark_initialized();
                }
                "Create a config (jarvy init)" => {
                    let options = commands::init::InitOptions::default();
                    let result = commands::init::run_init(options);
                    print!("{}", result.to_human());
                    // Mark as initialized after init
                    let _ = mark_initialized();
                }
                "Browse templates" => {
                    let result = commands::templates::list_templates();
                    println!("{}", result.to_human());
                }
                _ => {
                    println!("\nYou can always run these later:");
                    println!("  \x1b[36mjarvy quickstart\x1b[0m  - Guided setup");
                    println!("  \x1b[36mjarvy init\x1b[0m        - Create a config");
                    println!("  \x1b[36mjarvy templates\x1b[0m   - Browse templates\n");
                }
            },
            Err(_) => {
                println!("No choice was made");
            }
        }
        return;
    }

    // Normal flow for returning users
    print_logo();

    println!("\t\tHi, I'm Jarvy! I'm here to help you get your development environment set up.");

    let options = vec![
        "Run the project",
        "Test the project",
        "Development environment setup",
    ];

    let selection: Result<&str, InquireError> =
        Select::new("What would you like to do today?", options).prompt();

    match selection {
        Ok(choice) => {
            println!("selection: {}", choice);
            match choice {
                "Run the project" => {
                    println!("R");
                    // TODO set the override command in settings
                    match std::process::Command::new("cargo").arg("run").output() {
                        Ok(output) => {
                            // Handle the output here
                            println!("Output: {}", String::from_utf8_lossy(&output.stdout));
                        }
                        Err(e) => println!("Failed to execute command: {}", e),
                    }
                }
                "Test the project" => {
                    println!("T");
                    // TODO set the override command in settings
                    match std::process::Command::new("cargo").arg("test").output() {
                        Ok(output) => {
                            // Handle the output here
                            println!("Output: {}", String::from_utf8_lossy(&output.stdout));
                        }
                        Err(e) => println!("Failed to execute command: {}", e),
                    }
                }
                "Development environment setup" => {
                    // TODO set the override command in settings
                    println!("D");
                    setup();
                }
                _ => {}
            }
        }
        Err(_) => {
            println!("No choice was made")
        }
    }
}

/// Print the Jarvy logo banner
pub fn print_logo() {
    println!(
        "
 .----------------.
|   J A R V Y  ⚡   |
 '----------------'
    "
    );
}
