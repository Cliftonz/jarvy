use crate::analytics::init_logging;
use crate::config::{Config, create_default_config};
use crate::init::initialize;
use crate::setup::setup;
use clap::{Parser, Subcommand};
use inquire::{InquireError, Select};
use std::io::Write;

mod analytics;
mod config;
mod error_codes;
mod init;
mod os_setup;
mod outputs;
mod setup;
mod tests;
mod tools;

#[derive(Parser)]
#[clap(name = "jarvy", version = "1.0", author = "Zac Clifton")]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Set up the environment based on the configuration file
    Setup {
        /// Path to the configuration file
        #[clap(short, long, default_value = "./jarvy.toml")]
        file: String,
    },
    Configure {},
}

fn main() {
    let global_config = initialize();

    init_logging(global_config.telemetry);

    // Run the CLI Parser and commands
    let cli = Cli::parse();

    match &cli.command {
        Commands::Setup { file } => {
            let config = Config::new(file);

            setup();

            let tools = config.get_tool_configs();
            for (id, tool) in tools {
                println!(
                    "Installing {}: {} version {} using package manager: {}",
                    id, tool.name, tool.version, tool.version_manager
                );
                // Call the appropriate installer function here
            }
        }
        Commands::Configure {} => create_default_config(),
        _ => {
            user_select();
        }
    }
}

fn user_select() {
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

fn print_logo() {
    println!(
        "
 .----------------.
|   J A R V Y  ⚡   |
 '----------------'
    "
    );
}
