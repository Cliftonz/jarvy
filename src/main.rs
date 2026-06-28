//! Jarvy CLI - Development environment provisioning tool
//!
//! Process entry point: argument parsing, telemetry init, sandbox banner,
//! panic hook, command dispatch, OTLP flush. Command routing and handler
//! glue live in `crate::commands::dispatch` (PRD-037).

use clap::Parser;
use std::fs;

mod ai_hooks;
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
mod git_hooks;
mod hooks;
mod init;
pub mod interactive;
mod lock;
pub mod logging;
mod mcp;
mod mcp_register;
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
pub mod progress;
mod provisioner;
mod registry_remote;
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
use cli::{Cli, Commands};
use config::Config;
use init::initialize;

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
    // `JARVY_CI` / `JARVY_NO_CI` only inside `run_setup`, but the
    // telemetry-config merge below applies `sandbox::is_seamless_auto()`
    // → `crate::ci::is_ci()` → `cached_detect()` long before
    // `run_setup` runs. The cache locks in `None` from the no-env
    // baseline, so a subsequent `set_var("JARVY_CI", "1")` inside
    // `run_setup` is invisible to the cached state and the `Running
    // in CI mode` notice never fires. Hoist the forwarding here so
    // both early-init callers and the setup-command path see the
    // same forced-CI state.
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
    // override left the OTLP logger permanently off, while metrics still
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

    // Seamless / CI auto-disable, applied to the FINAL merged config.
    // `from_env` already encodes this on `env_config`, but `env_config`'s
    // `enabled` is only propagated above when `JARVY_TELEMETRY` is
    // explicitly set — which is the opposite of when the auto-disable
    // needs to fire. Under the opt-out default the disk value is `true`,
    // so without this re-application a CI / Codespaces / Claude-Code
    // sandbox would silently telemeter despite the documented contract
    // ("CI and unattended sandboxes auto-disable unless explicitly
    // overridden"). Forced sandbox (`JARVY_SANDBOX=1` without a real
    // detector match) is deliberately NOT in this gate — a hostile
    // dotfile or compromised devcontainer image that sets
    // `JARVY_SANDBOX=1` must not silently silence telemetry on a
    // victim's machine. Only `is_seamless_auto` (real detection)
    // triggers the disable.
    if std::env::var("JARVY_TELEMETRY").is_err() && sandbox::is_seamless_auto() {
        telemetry_config.enabled = false;
    }

    init_logging(&telemetry_config);
    telemetry::init(telemetry_config);

    // If `initialize_from_disk` rendered the first-run / legacy-upgrade
    // telemetry disclosure, emit the audit event now that the OTLP
    // layer is wired up. On-call uses this to confirm the disclosure
    // actually surfaced when a user files a privacy complaint.
    if let Some(trigger) = init::take_pending_disclosure() {
        telemetry::disclosure_shown(trigger);
    }

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
    let exit_code = commands::dispatch::run(&cli, &global_config);

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
