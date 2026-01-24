//! Jarvy CLI - Development environment provisioning tool
//!
//! This is the main entry point for the Jarvy CLI. Command logic has been
//! extracted to dedicated modules for maintainability (PRD-037).

use clap::{CommandFactory, Parser};
use std::fs;

mod analytics;
mod bootstrap;
mod ci;
pub mod cli;
mod commands;
mod config;
mod drift;
mod env;
mod error_codes;
mod git;
mod hooks;
mod init;
pub mod interactive;
mod lock;
mod mcp;
mod network;
mod observability;
mod onboarding;
mod os_setup;
mod output;
mod outputs;
mod packages;
mod provisioner;
pub mod remote;
mod report;
mod roles;
mod services;
mod setup;
mod team;
mod telemetry;
mod templates;
mod tools;
mod update;

use analytics::init_logging;
use cli::{Cli, Commands, parse_install_method, parse_update_channel};
use config::Config;
use init::initialize;
use onboarding::mark_initialized;
use output::Outputable;

fn main() {
    // Run the CLI Parser first so that -h/--help and -V/--version can exit without side effects
    let cli = Cli::parse();

    // If a user typed an unknown subcommand, handle it here (before any initialization)
    if let Some(Commands::External(args)) = &cli.command {
        if let Some(first) = args.first() {
            eprintln!("Unrecognized command: '{}'", first);
            eprintln!("Tip: run 'jarvy --help' to see available commands.");
        } else {
            eprintln!("Unrecognized command");
        }
        // Fall back to an interactive menu
        interactive::user_select();
        return;
    }

    // Initialize after parsing arguments
    let global_config = initialize();
    init_logging(global_config.settings.telemetry);

    // Initialize unified telemetry (OTEL-based)
    let mut telemetry_config = global_config.telemetry.clone();
    if !global_config.settings.telemetry {
        telemetry_config.enabled = false;
    }
    let env_config = telemetry::TelemetryConfig::from_env();
    if std::env::var("JARVY_TELEMETRY").is_ok() {
        telemetry_config.enabled = env_config.enabled;
    }
    if std::env::var("JARVY_OTLP_ENDPOINT").is_ok() {
        telemetry_config.endpoint = env_config.endpoint;
    }
    telemetry::init(telemetry_config);

    // Install panic hook
    std::panic::set_hook(Box::new(|info| {
        eprintln!("Jarvy panic: {}", info);
        tracing::error!(event = "panic", message = %info);
    }));

    // Telemetry smoke test
    if std::env::var("JARVY_TELEMETRY_SMOKE").as_deref() == Ok("1") {
        tracing::info!("telemetry smoke info");
        tracing::error!("telemetry smoke error");
        analytics::send_otlp_smoke_probe();
        std::thread::sleep(std::time::Duration::from_millis(800));
    }

    // Register built-in tools
    tools::register_all();

    // Dispatch to command handlers
    dispatch_command(&cli, &global_config);
}

/// Dispatch CLI commands to their handlers
fn dispatch_command(cli: &Cli, global_config: &init::CliConfig) {
    match &cli.command {
        Some(Commands::Setup {
            file,
            from,
            role,
            no_hooks,
            dry_run,
            ci,
            no_ci,
            jobs,
            sequential,
            ignore_missing_deps,
            insecure,
            header,
            ..
        }) => {
            commands::setup_cmd::run_setup(
                file,
                from.as_deref(),
                role.as_deref(),
                *no_hooks,
                *dry_run,
                *ci,
                *no_ci,
                *jobs,
                *sequential,
                *ignore_missing_deps,
                *insecure,
                header,
            );
        }
        Some(Commands::Bootstrap {}) => commands::run_bootstrap(),
        Some(Commands::Configure {}) => commands::run_configure(),
        Some(Commands::Get {
            file,
            output_format,
            output,
        }) => {
            commands::run_get(file, *output_format, output.as_deref());
        }
        Some(Commands::Tools {
            index,
            default_hooks,
            output_format,
            output,
        }) => {
            commands::run_tools(*index, *default_hooks, *output_format, output.as_deref());
        }
        Some(Commands::Env {
            file,
            dotenv,
            shell,
            dry_run,
            export,
            shell_type,
            force,
        }) => {
            commands::run_env(
                file,
                *dotenv,
                *shell,
                *dry_run,
                *export,
                shell_type.as_deref(),
                *force,
            );
        }
        Some(Commands::CiConfig {
            provider,
            output,
            dry_run,
        }) => {
            commands::run_ci_config(*provider, output, *dry_run);
        }
        Some(Commands::CiInfo {}) => commands::run_ci_info(),
        Some(Commands::Services { action, file }) => {
            commands::run_services(action, file);
        }
        Some(Commands::Doctor {
            file,
            tools,
            output_format,
            extended,
            report,
        }) => {
            handle_doctor(file, tools, output_format, *extended, report);
        }
        Some(Commands::Diff {
            file,
            changes_only,
            output_format,
        }) => {
            handle_diff(file, *changes_only, output_format);
        }
        Some(Commands::Export {
            tools,
            all,
            verbose,
            output_format,
            output,
        }) => {
            handle_export(tools, *all, *verbose, output_format, output);
        }
        Some(Commands::Upgrade {
            file,
            tools,
            dry_run,
            force,
            output_format,
        }) => {
            handle_upgrade(file, tools, *dry_run, *force, output_format);
        }
        Some(Commands::Init {
            template,
            non_interactive,
            stdout,
            output,
        }) => {
            handle_init(template, *non_interactive, *stdout, output);
        }
        Some(Commands::Search {
            query,
            all,
            output_format,
        }) => {
            handle_search(query, *all, output_format);
        }
        Some(Commands::Validate {
            file,
            from,
            strict,
            header,
            output_format,
        }) => {
            handle_validate(file, from, *strict, header, output_format);
        }
        Some(Commands::Completions {
            shell,
            instructions,
        }) => {
            handle_completions(shell, *instructions);
        }
        Some(Commands::Templates { action }) => {
            handle_templates(action);
        }
        Some(Commands::Quickstart {
            non_interactive,
            skip_check,
        }) => {
            handle_quickstart(*non_interactive, *skip_check);
        }
        Some(Commands::Telemetry { action }) => {
            commands::run_telemetry(action, global_config);
        }
        Some(Commands::Mcp { config }) => commands::run_mcp(config.clone()),
        Some(Commands::Diagnose {
            tool,
            fix,
            export,
            scope,
            output_format,
        }) => {
            commands::diagnose::run_diagnose(tool, *fix, *export, scope, output_format);
        }
        Some(Commands::Team { action }) => commands::run_team(action),
        Some(Commands::Roles { file, action }) => commands::run_roles(file, action),
        Some(Commands::Lock { action }) => commands::run_lock(action),
        Some(Commands::Config { action }) => commands::run_config(action),
        Some(Commands::Update {
            action,
            version,
            channel,
            method,
            rollback,
        }) => {
            handle_update(action, version, channel, method, *rollback);
        }
        Some(Commands::Drift { file, action }) => commands::run_drift(file, action),
        None => interactive::user_select(),
        Some(Commands::External(_)) => unreachable!("External subcommand handled before init"),
    }
}

// Helper functions for commands that need inline handling

fn handle_doctor(
    file: &Option<String>,
    tools: &Option<String>,
    output_format: &str,
    extended: bool,
    report: &Option<String>,
) {
    let config = file.as_ref().map(|f| Config::new(f));
    let specific_tools = tools.as_ref().map(|t| {
        t.split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()
    });

    if extended {
        let result = commands::doctor::run_doctor_extended(config.as_ref(), specific_tools);
        if let Some(report_path) = report {
            if let Err(e) = commands::doctor::export_report(&result, report_path) {
                eprintln!("Failed to export report: {}", e);
            } else {
                println!("Report exported to: {}", report_path);
            }
        }
        let output = if output_format == "json" {
            serde_json::to_string_pretty(&result)
                .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
        } else {
            result.to_human()
        };
        println!("{}", output);
        std::process::exit(result.exit_code().code());
    } else {
        let result = commands::doctor::run_doctor(config.as_ref(), specific_tools);
        let output = if output_format == "json" {
            serde_json::to_string_pretty(&result)
                .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
        } else {
            result.to_human()
        };
        println!("{}", output);
        std::process::exit(result.exit_code().code());
    }
}

fn handle_diff(file: &str, changes_only: bool, output_format: &str) {
    let config = Config::new(file);
    let result = commands::diff::run_diff(&config, changes_only);
    let output = if output_format == "json" {
        serde_json::to_string_pretty(&result).unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
    } else {
        result.to_human()
    };
    println!("{}", output);
    std::process::exit(result.exit_code().code());
}

fn handle_export(
    tools: &Option<String>,
    all: bool,
    verbose: bool,
    output_format: &str,
    output: &Option<String>,
) {
    let filter_tools = tools.as_ref().map(|t| {
        t.split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()
    });
    let result = commands::export::export_tools(filter_tools, all, verbose);
    let content = if output_format == "json" {
        result.to_json()
    } else {
        result.to_human()
    };
    if let Some(path) = output {
        if let Err(e) = fs::write(path, &content) {
            eprintln!("Failed to write output: {}", e);
            std::process::exit(1);
        }
        println!("Exported to: {}", path);
    } else {
        println!("{}", content);
    }
    std::process::exit(result.exit_code().code());
}

fn handle_upgrade(
    file: &Option<String>,
    tools: &Option<String>,
    dry_run: bool,
    force: bool,
    output_format: &str,
) {
    let config = file.as_ref().map(|f| Config::new(f));
    let specific_tools = tools.as_ref().map(|t| {
        t.split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()
    });
    let result = commands::upgrade::run_upgrade(config.as_ref(), specific_tools, dry_run, force);
    let output = if output_format == "json" {
        serde_json::to_string_pretty(&result).unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
    } else {
        result.to_human()
    };
    println!("{}", output);
    std::process::exit(result.exit_code().code());
}

fn handle_init(
    template: &Option<String>,
    non_interactive: bool,
    stdout: bool,
    output: &Option<String>,
) {
    let options = commands::init::InitOptions {
        template: template.clone(),
        non_interactive,
        stdout,
        output: output.as_ref().map(std::path::PathBuf::from),
    };
    let result = commands::init::run_init(options);
    let content = result.to_human();
    if !content.is_empty() {
        print!("{}", content);
    }
    std::process::exit(result.exit_code().code());
}

fn handle_search(query: &Option<String>, all: bool, output_format: &str) {
    let query_str = query.as_deref().unwrap_or("");
    let result = commands::search::search_tools(query_str, all);
    let output = if output_format == "json" {
        serde_json::to_string_pretty(&result).unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
    } else {
        result.to_human()
    };
    println!("{}", output);
    std::process::exit(result.exit_code().code());
}

fn handle_validate(
    file: &str,
    from: &Option<String>,
    strict: bool,
    header: &[String],
    output_format: &str,
) {
    let config_path = if let Some(url) = from {
        match remote::fetch_remote_config(url, false, header) {
            Ok(path) => path,
            Err(e) => {
                eprintln!("Error fetching remote config: {}", e);
                std::process::exit(error_codes::CONFIG_ERROR);
            }
        }
    } else {
        file.to_string()
    };
    let result = commands::validate::validate_config(&config_path, strict);
    let output = if output_format == "json" {
        serde_json::to_string_pretty(&result).unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
    } else {
        result.to_human()
    };
    println!("{}", output);
    std::process::exit(result.exit_code().code());
}

fn handle_completions(shell: &str, instructions: bool) {
    if instructions {
        println!("{}", commands::completions::get_install_instructions());
        return;
    }
    let shell_type: commands::completions::CompletionShell = match shell.parse() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };
    let mut cmd = Cli::command();
    let completions = commands::completions::generate_completions_string(&mut cmd, shell_type);
    println!("{}", completions);
}

fn handle_templates(action: &cli::TemplatesSubcommand) {
    match action {
        cli::TemplatesSubcommand::List {} => {
            let result = commands::templates::list_templates();
            println!("{}", result.to_human());
            std::process::exit(result.exit_code().code());
        }
        cli::TemplatesSubcommand::Show { name } => {
            let result = commands::templates::show_template(name);
            println!("{}", result.to_human());
            std::process::exit(result.exit_code().code());
        }
        cli::TemplatesSubcommand::Use {
            name,
            output,
            setup,
        } => {
            let output_path = output.as_ref().map(std::path::PathBuf::from);
            let result = commands::templates::use_template(name, output_path);
            println!("{}", result.to_human());
            if *setup && result.created {
                println!("\nRunning setup...\n");
            }
            std::process::exit(result.exit_code().code());
        }
    }
}

fn handle_quickstart(non_interactive: bool, skip_check: bool) {
    let options = commands::quickstart::QuickstartOptions {
        non_interactive,
        skip_check,
    };
    let result = commands::quickstart::run_quickstart(options);
    println!("{}", result.to_human());
    if !result.aborted {
        let _ = mark_initialized();
    }
    std::process::exit(result.exit_code().code());
}

fn handle_update(
    action: &Option<cli::UpdateSubcommand>,
    version: &Option<String>,
    channel: &Option<String>,
    method: &Option<String>,
    rollback: bool,
) {
    let update_action = match action {
        Some(cli::UpdateSubcommand::Check { channel: ch }) => {
            let ch = ch.as_ref().or(channel.as_ref());
            update::UpdateAction::Check {
                channel: ch.and_then(|c| parse_update_channel(c)),
            }
        }
        Some(cli::UpdateSubcommand::History {}) => update::UpdateAction::History,
        Some(cli::UpdateSubcommand::Config {}) => update::UpdateAction::Config,
        Some(cli::UpdateSubcommand::Enable {}) => update::UpdateAction::Enable,
        Some(cli::UpdateSubcommand::Disable {}) => update::UpdateAction::Disable,
        None => update::UpdateAction::Install {
            version: version.clone(),
            channel: channel.as_ref().and_then(|c| parse_update_channel(c)),
            method: method.as_ref().and_then(|m| parse_install_method(m)),
            rollback,
        },
    };
    std::process::exit(update::run_update_command(update_action));
}
