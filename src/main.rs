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
pub mod logging;
mod mcp;
mod meta;
mod net;
mod network;
mod observability;
mod onboarding;
mod os_setup;
mod output;
mod outputs;
mod packages;
mod paths;
mod provisioner;
pub mod remote;
mod report;
mod roles;
mod sandbox;
mod security;
mod services;
mod setup;
pub mod shell_init;
mod team;
mod telemetry;
mod templates;
pub mod ticket;
mod tools;
mod update;
mod workspace;

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

    // CI-flag forwarding to env BEFORE any cached ci::detect()/is_ci()
    // call. The `Setup { ci, no_ci, .. }` flags were forwarded into
    // `JARVY_CI` / `JARVY_NO_CI` only inside `run_setup` — but
    // `telemetry::init` (and `update::config`) call
    // `sandbox::is_seamless_auto()` → `crate::ci::is_ci()` →
    // `cached_detect()` long before `run_setup` runs. The cache locks
    // in `None` from the no-env baseline, so the subsequent
    // `set_var("JARVY_CI", "1")` inside `run_setup` is invisible to the
    // cached state and the `Running in CI mode` notice never fires.
    // Hoist the forwarding here so both early-init callers and the
    // setup-command path see the same forced-CI state.
    //
    // SAFETY: env vars set at startup before any threads are spawned.
    #[allow(unsafe_code)]
    {
        if let Some(Commands::Setup { ci, no_ci, .. }) = &cli.command {
            if *ci {
                unsafe { std::env::set_var("JARVY_CI", "1") };
            } else if *no_ci {
                unsafe { std::env::set_var("JARVY_NO_CI", "1") };
            }
        }
    }

    // Initialize after parsing arguments
    let global_config = initialize();

    // Build the effective TelemetryConfig BEFORE `init_logging` so the
    // tracing subscriber's OTLP layer sees the same merge result as
    // `telemetry::init` (metrics + traces). Earlier versions gated the
    // log layer on the file flag only — `JARVY_TELEMETRY=1` env-only
    // opt-in left the OTLP logger permanently off, while metrics still
    // exported, producing a half-on telemetry stack that was hard to
    // diagnose.
    //
    // Precedence: env > project jarvy.toml > global ~/.jarvy/config.toml.
    let mut telemetry_config = global_config.telemetry.clone();
    if !global_config.settings.telemetry {
        // Legacy opt-out flag — preserved for users who set it via the
        // old `[settings] telemetry = false` shape before the migration.
        telemetry_config.enabled = false;
    }

    // Apply project-level telemetry config from jarvy.toml (if present).
    // Trust-boundary policy lives in `TelemetryConfig::narrow_with_project`
    // — kept there so it's table-testable without spinning up `main`.
    let project_config_path = extract_config_path(&cli);
    if let Some(ref path) = project_config_path {
        if let Ok(contents) = fs::read_to_string(path) {
            if let Ok(project_config) = toml::from_str::<Config>(&contents) {
                if let Some(project_telemetry) = project_config.telemetry {
                    if let Some(warning) = telemetry_config.narrow_with_project(&project_telemetry)
                    {
                        eprintln!("{}", warning);
                    }
                }
            }
        }
    }

    // Env vars take highest priority (override both global and project)
    let env_config = telemetry::TelemetryConfig::from_env();
    if std::env::var("JARVY_TELEMETRY").is_ok() {
        telemetry_config.enabled = env_config.enabled;
    }
    if std::env::var("JARVY_OTLP_ENDPOINT").is_ok() {
        telemetry_config.endpoint = env_config.endpoint;
    }

    init_logging(&telemetry_config);
    telemetry::init(telemetry_config);

    // Install panic hook BEFORE the banner so any (currently hard-to-
    // hit) stderr-emission failure produces a structured panic
    // message instead of a default Rust backtrace dump.
    std::panic::set_hook(Box::new(|info| {
        eprintln!("Jarvy panic: {}", info);
        tracing::error!(event = "panic", message = %info);
    }));

    // Seamless-mode banner: if Jarvy detects an AI sandbox or
    // long-running container env, surface one line so the operator
    // understands why prompts/telemetry/update checks are off.
    // Suppressed when the invocation is already quiet or asks for
    // structured output (which goes to stdout — keep stderr quiet).
    // The tracing event in `sandbox::detect()` fires regardless of
    // muting, so jarvy.log records the decision even for `--json`
    // consumers. See PRD-053.
    if let Some(sb) = sandbox::detect() {
        // Walk argv with peek so `--format json` (space form) is
        // recognised alongside `--format=json` (equals form).
        let mut muted = std::env::var("JARVY_QUIET").as_deref() == Ok("1");
        if !muted {
            let mut prev_was_format_flag = false;
            for a in std::env::args() {
                if a == "--quiet"
                    || a == "-q"
                    || a == "--json"
                    || a.starts_with("--format=json")
                    || a.starts_with("--log-format=json")
                {
                    muted = true;
                    break;
                }
                if prev_was_format_flag && a == "json" {
                    muted = true;
                    break;
                }
                prev_was_format_flag = a == "--format" || a == "--log-format";
            }
        }
        if !muted {
            sandbox::print_banner_once(&sb);
        }
    }

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
    let exit_code = dispatch_command(&cli, &global_config);

    // Flush OTLP signal batches before `process::exit` kills worker
    // threads — `exit` skips every `Drop`, including the batch
    // processor's shutdown sequence (round-2 obs P0). Both providers
    // must be drained explicitly: `telemetry::shutdown` flushes the
    // PeriodicReader's metric batch (60s default cadence — without
    // explicit shutdown short-lived commands like `jarvy setup` exit
    // before the first periodic export and lose every metric point);
    // `analytics::shutdown_logging` drains the BatchLogProcessor's
    // log queue.
    telemetry::shutdown();
    analytics::shutdown_logging();

    std::process::exit(exit_code);
}

/// Dispatch CLI commands to their handlers.
/// Returns the process exit code.
fn dispatch_command(cli: &Cli, global_config: &init::CliConfig) -> i32 {
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
            header,
            ..
        }) => commands::setup_cmd::run_setup(
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
            header,
            global_config.settings.fingerprint.as_deref(),
        ),
        Some(Commands::Bootstrap {}) => {
            commands::run_bootstrap();
            0
        }
        Some(Commands::Configure {}) => {
            commands::run_configure();
            0
        }
        Some(Commands::Get {
            file,
            output_format,
            output,
        }) => {
            commands::run_get(file, *output_format, output.as_deref());
            0
        }
        Some(Commands::Tools {
            index,
            default_hooks,
            request,
            open,
            output_format,
            output,
        }) => commands::run_tools(
            *index,
            *default_hooks,
            request.as_deref(),
            *open,
            *output_format,
            output.as_deref(),
        ),
        Some(Commands::Env {
            file,
            dotenv,
            shell,
            dry_run,
            export,
            shell_type,
            force,
        }) => commands::run_env(
            file,
            *dotenv,
            *shell,
            *dry_run,
            *export,
            shell_type.as_deref(),
            *force,
        ),
        Some(Commands::CiConfig {
            provider,
            output,
            dry_run,
        }) => commands::run_ci_config(*provider, output, *dry_run),
        Some(Commands::CiInfo {}) => {
            commands::run_ci_info();
            0
        }
        Some(Commands::Services { action, file }) => commands::run_services(action, file),
        Some(Commands::Doctor {
            file,
            tools,
            output_format,
            extended,
            report,
        }) => handle_doctor(file, tools, output_format, *extended, report),
        Some(Commands::Diff {
            file,
            changes_only,
            output_format,
        }) => handle_diff(file, *changes_only, output_format),
        Some(Commands::Export {
            tools,
            all,
            verbose,
            output_format,
            output,
        }) => handle_export(tools, *all, *verbose, output_format, output),
        Some(Commands::Upgrade {
            file,
            tools,
            dry_run,
            force,
            output_format,
        }) => handle_upgrade(file, tools, *dry_run, *force, output_format),
        Some(Commands::Init {
            template,
            non_interactive,
            stdout,
            output,
        }) => handle_init(template, *non_interactive, *stdout, output),
        Some(Commands::Search {
            query,
            all,
            output_format,
        }) => handle_search(query, *all, output_format),
        Some(Commands::Validate {
            file,
            from,
            strict,
            header,
            output_format,
        }) => handle_validate(file, from, *strict, header, output_format),
        Some(Commands::Completions {
            shell,
            instructions,
        }) => handle_completions(shell, *instructions),
        Some(Commands::Templates { action }) => handle_templates(action),
        Some(Commands::Quickstart {
            non_interactive,
            skip_check,
        }) => handle_quickstart(*non_interactive, *skip_check),
        Some(Commands::Telemetry { action }) => {
            commands::run_telemetry(action, global_config);
            0
        }
        Some(Commands::Mcp { config }) => commands::run_mcp(config.clone()),
        Some(Commands::Diagnose {
            tool,
            fix,
            export,
            scope,
            output_format,
        }) => commands::diagnose::run_diagnose(tool, *fix, *export, scope, output_format),
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
            allow_unsigned,
        }) => handle_update(action, version, channel, method, *rollback, *allow_unsigned),
        Some(Commands::Drift { file, action }) => commands::run_drift(file, action),
        Some(Commands::ShellInit { shell }) => {
            commands::shell_init_cmd::run_shell_init(shell.as_deref())
        }
        Some(Commands::Ensure {
            force,
            quiet,
            foreground,
        }) => commands::ensure_cmd::run_ensure(*force, *quiet, *foreground),
        Some(Commands::Logs { action }) => commands::run_logs_command(action.clone()),
        Some(Commands::Ticket { action }) => commands::run_ticket_command(action.clone()),
        Some(Commands::Explain {
            tool,
            file,
            output_format,
        }) => handle_explain(tool, file, output_format),
        Some(Commands::Audit {
            tool,
            output_format,
        }) => handle_audit(tool, output_format),
        Some(Commands::Migrate {
            file,
            apply,
            output_format,
        }) => handle_migrate(file, *apply, output_format),
        Some(Commands::Schema { output }) => handle_schema(output),
        None => {
            interactive::user_select();
            0
        }
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
) -> i32 {
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
        crate::output::print_and_exit(result, output_format)
    } else {
        let result = commands::doctor::run_doctor(config.as_ref(), specific_tools);
        crate::output::print_and_exit(result, output_format)
    }
}

fn handle_diff(file: &str, changes_only: bool, output_format: &str) -> i32 {
    let config = Config::new(file);
    let result = commands::diff::run_diff(&config, changes_only);
    crate::output::print_and_exit(result, output_format)
}

fn handle_export(
    tools: &Option<String>,
    all: bool,
    verbose: bool,
    output_format: &str,
    output: &Option<String>,
) -> i32 {
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
            return 1;
        }
        println!("Exported to: {}", path);
    } else {
        println!("{}", content);
    }
    result.exit_code().code()
}

fn handle_upgrade(
    file: &Option<String>,
    tools: &Option<String>,
    dry_run: bool,
    force: bool,
    output_format: &str,
) -> i32 {
    let config = file.as_ref().map(|f| Config::new(f));
    let specific_tools = tools.as_ref().map(|t| {
        t.split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()
    });
    let result = commands::upgrade::run_upgrade(config.as_ref(), specific_tools, dry_run, force);
    crate::output::print_and_exit(result, output_format)
}

fn handle_init(
    template: &Option<String>,
    non_interactive: bool,
    stdout: bool,
    output: &Option<String>,
) -> i32 {
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
    result.exit_code().code()
}

fn handle_search(query: &Option<String>, all: bool, output_format: &str) -> i32 {
    let query_str = query.as_deref().unwrap_or("");
    let result = commands::search::search_tools(query_str, all);
    crate::output::print_and_exit(result, output_format)
}

fn handle_validate(
    file: &str,
    from: &Option<String>,
    strict: bool,
    header: &[String],
    output_format: &str,
) -> i32 {
    let config_path = if let Some(url) = from {
        match remote::fetch_remote_config(url, header) {
            Ok(path) => path,
            Err(e) => {
                eprintln!("Error fetching remote config: {}", e);
                return error_codes::CONFIG_ERROR;
            }
        }
    } else {
        file.to_string()
    };
    let result = commands::validate::validate_config(&config_path, strict);
    crate::output::print_and_exit(result, output_format)
}

fn handle_completions(shell: &str, instructions: bool) -> i32 {
    if instructions {
        println!("{}", commands::completions::get_install_instructions());
        return 0;
    }
    let shell_type: commands::completions::CompletionShell = match shell.parse() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {}", e);
            return 1;
        }
    };
    let mut cmd = Cli::command();
    let completions = commands::completions::generate_completions_string(&mut cmd, shell_type);
    println!("{}", completions);
    0
}

fn handle_templates(action: &cli::TemplatesSubcommand) -> i32 {
    match action {
        cli::TemplatesSubcommand::List {} => {
            let result = commands::templates::list_templates();
            println!("{}", result.to_human());
            result.exit_code().code()
        }
        cli::TemplatesSubcommand::Show { name } => {
            let result = commands::templates::show_template(name);
            println!("{}", result.to_human());
            result.exit_code().code()
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
            result.exit_code().code()
        }
    }
}

fn handle_quickstart(non_interactive: bool, skip_check: bool) -> i32 {
    let options = commands::quickstart::QuickstartOptions {
        non_interactive,
        skip_check,
    };
    let result = commands::quickstart::run_quickstart(options);
    println!("{}", result.to_human());
    if !result.aborted {
        let _ = mark_initialized();
    }
    result.exit_code().code()
}

fn handle_update(
    action: &Option<cli::UpdateSubcommand>,
    version: &Option<String>,
    channel: &Option<String>,
    method: &Option<String>,
    rollback: bool,
    allow_unsigned: bool,
) -> i32 {
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
            allow_unsigned,
        },
    };
    update::run_update_command(update_action)
}

fn handle_explain(tool: &str, file: &Option<String>, output_format: &str) -> i32 {
    let result = commands::explain::run_explain(tool, file.as_deref());
    crate::output::print_and_exit(result, output_format)
}

fn handle_audit(tool: &Option<String>, output_format: &str) -> i32 {
    let result = commands::audit::run_audit(tool.as_deref());
    crate::output::print_and_exit(result, output_format)
}

fn handle_migrate(file: &str, apply: bool, output_format: &str) -> i32 {
    if apply {
        eprintln!(
            "Error: --apply is not yet implemented. The current `jarvy migrate` only reports \
             suggested rewrites; apply them by hand. Re-run without --apply to see the report."
        );
        return error_codes::CONFIG_ERROR;
    }
    let result = commands::migrate::run_migrate(file, apply);
    crate::output::print_and_exit(result, output_format)
}

fn handle_schema(output: &Option<String>) -> i32 {
    let result = commands::schema::generate_schema();
    let content = result.to_human();
    if let Some(path) = output {
        if let Err(e) = fs::write(path, &content) {
            eprintln!("Failed to write schema: {}", e);
            return 1;
        }
        println!("Schema written to: {}", path);
    } else {
        println!("{}", content);
    }
    0
}

/// Extract the config file path from the CLI command (for early telemetry config loading).
/// Defaults to `./jarvy.toml` if the command doesn't specify a file.
fn extract_config_path(cli: &Cli) -> Option<String> {
    match &cli.command {
        Some(Commands::Setup { file, .. })
        | Some(Commands::Get { file, .. })
        | Some(Commands::Env { file, .. })
        | Some(Commands::Diff { file, .. })
        | Some(Commands::Validate { file, .. })
        | Some(Commands::Roles { file, .. })
        | Some(Commands::Drift { file, .. })
        | Some(Commands::Services { file, .. }) => Some(file.clone()),
        _ => {
            // Try default path for commands that don't have a --file flag
            let default = "./jarvy.toml";
            if std::path::Path::new(default).exists() {
                Some(default.to_string())
            } else {
                None
            }
        }
    }
}
