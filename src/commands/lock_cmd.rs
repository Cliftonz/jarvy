//! Lock command handler - manage version lock files

use std::path::Path;

use crate::cli::LockAction;
use crate::config;
use crate::lock;

/// Handle lock subcommands
pub fn run_lock(action: &LockAction) {
    match action {
        LockAction::Generate { file, output } => {
            let config = config::Config::new(file);
            let tools = config.get_tool_configs();

            println!("Generating lock file from {}...", file);

            match lock::generate_lock(&tools) {
                Ok(lock_file) => {
                    let path = Path::new(output);
                    match lock_file.save(path) {
                        Ok(()) => {
                            println!("Lock file generated: {}", output);
                            println!("  Tools locked: {}", lock_file.tools.len());
                        }
                        Err(e) => {
                            eprintln!("Failed to save lock file: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to generate lock file: {}", e);
                    std::process::exit(1);
                }
            }
        }
        LockAction::Status { lock_file, verbose } => {
            let path = Path::new(lock_file);
            if !path.exists() {
                eprintln!("Lock file not found: {}", lock_file);
                eprintln!("Generate one with: jarvy lock generate");
                std::process::exit(1);
            }

            match lock::LockFile::load(path) {
                Ok(lock) => {
                    let platform = std::env::consts::OS;
                    let result = lock::verify_lock(&lock, platform);

                    println!("Lock File Status");
                    println!("================");
                    println!("File: {}", lock_file);
                    println!("Version: {}", lock.version);
                    println!("Tools: {}", lock.tools.len());
                    println!();

                    if *verbose {
                        for tool in &result.tools {
                            let status_icon = match tool.status {
                                lock::VerificationStatus::Match => "✓",
                                lock::VerificationStatus::VersionMismatch => "✗",
                                lock::VerificationStatus::NotInstalled => "○",
                                lock::VerificationStatus::NotLocked => "?",
                                lock::VerificationStatus::Unknown => "?",
                            };
                            let installed = tool.installed_version.as_deref().unwrap_or("-");
                            println!(
                                "  {} {} (locked: {}, installed: {})",
                                status_icon, tool.name, tool.locked_version, installed
                            );
                        }
                        println!();
                    }

                    println!(
                        "Summary: {} matched, {} mismatched, {} missing",
                        result.matched, result.mismatched, result.missing
                    );

                    if result.all_match {
                        println!("Status: All tools match lock file ✓");
                    } else {
                        println!("Status: Some tools differ from lock file");
                    }
                }
                Err(e) => {
                    eprintln!("Failed to load lock file: {}", e);
                    std::process::exit(1);
                }
            }
        }
        LockAction::Verify {
            lock_file,
            output_format,
        } => {
            let path = Path::new(lock_file);
            if !path.exists() {
                eprintln!("Lock file not found: {}", lock_file);
                std::process::exit(1);
            }

            match lock::LockFile::load(path) {
                Ok(lock) => {
                    let platform = std::env::consts::OS;
                    let result = lock::verify_lock(&lock, platform);

                    if output_format == "json" {
                        // JSON output
                        let output: Vec<serde_json::Value> = result
                            .tools
                            .iter()
                            .map(|t| {
                                serde_json::json!({
                                    "name": t.name,
                                    "status": t.status.to_string(),
                                    "locked_version": t.locked_version,
                                    "installed_version": t.installed_version,
                                })
                            })
                            .collect();
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&output).unwrap_or_default()
                        );
                    } else {
                        // Pretty output
                        for tool in &result.tools {
                            let color = match tool.status {
                                lock::VerificationStatus::Match => "\x1b[32m",
                                lock::VerificationStatus::VersionMismatch => "\x1b[33m",
                                lock::VerificationStatus::NotInstalled => "\x1b[31m",
                                _ => "\x1b[90m",
                            };
                            let reset = "\x1b[0m";
                            let installed = tool.installed_version.as_deref().unwrap_or("-");
                            println!(
                                "{}{}{}: locked={}, installed={} [{}]",
                                color,
                                tool.name,
                                reset,
                                tool.locked_version,
                                installed,
                                tool.status
                            );
                        }
                    }

                    if !result.all_match {
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to load lock file: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
