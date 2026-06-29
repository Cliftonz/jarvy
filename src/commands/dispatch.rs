//! CLI dispatch and thin command-handler wrappers
//!
//! Extracted from `src/main.rs` to keep the binary entry point focused on
//! process initialization (telemetry, sandbox detection, panic hook, OTLP
//! flush at exit). This module owns the `Cli` → handler routing plus the
//! glue handlers for commands whose run() function needs minor argument
//! shaping (parsing `Option<String>` → typed enum, splitting a
//! comma-separated string, writing output to a file vs stdout) before
//! delegating into the per-command module.
//!
//! Anything more elaborate than ~10 lines of glue belongs in its own
//! `src/commands/<name>_cmd.rs` module — these wrappers are deliberately
//! the minimum needed to keep the dispatch table readable.

use crate::cli::{self, Cli, Commands, parse_install_method, parse_update_channel};
use crate::commands;
use crate::config::Config;
use crate::error_codes;
use crate::init;
use crate::interactive;
use crate::onboarding::mark_initialized;
use crate::output::Outputable;
use crate::remote;
use crate::update;
use clap::CommandFactory;
use std::fs;

/// Dispatch CLI commands to their handlers. Returns the process exit
/// code; the caller is responsible for OTLP flush + `process::exit`.
pub fn run(cli: &Cli, global_config: &init::CliConfig) -> i32 {
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
        Some(Commands::CiInfo { output_format }) => {
            commands::run_ci_info(output_format);
            0
        }
        Some(Commands::Discover {
            file,
            apply,
            missing,
            output_format,
        }) => crate::discover::commands::run_discover(file, *apply, *missing, output_format),
        Some(Commands::Workspace { file, action }) => commands::run_workspace(action, file),
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
        Some(Commands::Registry { action }) => commands::registry_cmd::run_registry(action),
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
        Some(Commands::AiHooks { action, file }) => commands::run_ai_hooks(action, file),
        Some(Commands::McpRegister { action, file }) => commands::run_mcp_register(action, file),
        Some(Commands::Hooks { action, file }) => commands::run_hooks(action, file),
        Some(Commands::Skills { action, file }) => commands::run_skills(action, file),
        None => {
            interactive::user_select();
            0
        }
        Some(Commands::External(_)) => unreachable!("External subcommand handled before init"),
    }
}

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
