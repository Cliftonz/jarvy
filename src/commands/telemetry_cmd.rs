//! Telemetry command handler - manage telemetry settings

use crate::cli::TelemetryAction;
use crate::init;
use crate::telemetry;

/// Handle telemetry subcommands
pub fn run_telemetry(action: &TelemetryAction, global_config: &init::CliConfig) {
    match action {
        TelemetryAction::Status {} => {
            let config = telemetry::config();
            println!("Telemetry Configuration");
            println!("=======================");
            if let Some(cfg) = config {
                println!(
                    "Status:    {}",
                    if cfg.is_enabled() {
                        "\x1b[32menabled\x1b[0m"
                    } else {
                        "\x1b[33mdisabled\x1b[0m"
                    }
                );
                println!(
                    "Endpoint:  {} ({})",
                    cfg.endpoint,
                    cfg.protocol.to_uppercase()
                );
                println!(
                    "Signals:   logs={}, metrics={}, traces={}",
                    if cfg.logs { "on" } else { "off" },
                    if cfg.metrics { "on" } else { "off" },
                    if cfg.traces { "on" } else { "off" }
                );
                println!("Sample:    {}%", (cfg.sample_rate * 100.0) as u32);
            } else {
                println!("Status:    \x1b[33mnot initialized\x1b[0m");
            }
            // Surface OTEL bootstrap state so a degraded exporter is visible
            // here instead of only as a single eprintln! at startup.
            match crate::analytics::telemetry_bootstrap_state() {
                crate::analytics::TelemetryBootstrapState::Healthy => {
                    println!("Exporter:  \x1b[32mhealthy\x1b[0m");
                }
                crate::analytics::TelemetryBootstrapState::Disabled => {
                    println!("Exporter:  disabled");
                }
                crate::analytics::TelemetryBootstrapState::Degraded => {
                    println!(
                        "Exporter:  \x1b[31mdegraded\x1b[0m \
                         (OTLP failed to initialize — see startup log for reason)"
                    );
                }
            }
            println!();

            // Show machine fingerprint
            let fp = global_config
                .settings
                .fingerprint
                .as_deref()
                .unwrap_or("not set");
            println!("Machine ID: {}", fp);
            println!("  This is a one-way hash of hardware identifiers (CPU, OS, disk serial).");
            println!("  It cannot be reversed to recover your hardware details.");
            println!("  Run `jarvy telemetry disable` to clear it.");
            println!();
            println!("Configuration sources:");
            println!("  - Config file: ~/.jarvy/config.toml [telemetry] section");
            println!("  - Environment: JARVY_TELEMETRY, JARVY_OTLP_ENDPOINT");
            println!("  - Privacy details: https://github.com/Cliftonz/jarvy/blob/main/PRIVACY.md");
        }
        TelemetryAction::Enable {} => {
            update_telemetry_config(true, None);
            println!("Telemetry enabled.");
            println!("Configure endpoint with: jarvy telemetry set-endpoint <url>");
        }
        TelemetryAction::Disable {} => {
            update_telemetry_config(false, None);
            clear_machine_fingerprint();
            println!("Telemetry disabled. Machine fingerprint cleared.");
        }
        TelemetryAction::SetEndpoint { url } => {
            update_telemetry_config(true, Some(url.clone()));
            println!("Endpoint set to: {}", url);
        }
        TelemetryAction::Test {} => {
            let config = telemetry::config();
            if let Some(cfg) = config {
                if !cfg.is_enabled() {
                    println!("Telemetry is disabled. Enable with: jarvy telemetry enable");
                    return;
                }
                println!("Sending test event to {}...", cfg.endpoint);
                telemetry::command_executed(
                    "telemetry_test",
                    std::time::Duration::from_millis(1),
                    true,
                );
                // Give exporters a moment to ship
                std::thread::sleep(std::time::Duration::from_millis(500));
                println!("Test event sent. Check your OTEL backend for:");
                println!("  - Event: command.executed");
                println!("  - Command: telemetry_test");
            } else {
                println!("Telemetry not initialized.");
            }
        }
        TelemetryAction::Preview {} => {
            println!("Telemetry Events Preview");
            println!("========================");
            println!();
            println!("On next setup, the following events would be sent:");
            println!();
            println!("Tool Events:");
            println!("  - tool.requested        (per tool in config)");
            println!("  - tool.installed        (for each successful install)");
            println!("  - tool.failed           (for each failed install)");
            println!("  - tool.unsupported      (per unknown tool — emitted by setup");
            println!("                            AND by `jarvy tools --request`)");
            println!();
            println!("    `tool.unsupported` field shape (uniform across call sites):");
            println!("      tool, version?, source, platform, suggestions, channel,");
            println!("      fallback_issue_url, scaffold_cmd, exit_code,");
            println!("      opt_in_bypassed (true only for --request path)");
            println!("    Channel values: \"telemetry\" | \"manual\"");
            println!();
            println!("Setup Events:");
            println!("  - setup.started    (when setup begins)");
            println!("  - setup.completed  (summary with counts/duration)");
            println!();
            println!("Hook Events:");
            println!("  - hook.started     (when hook begins)");
            println!("  - hook.completed   (on success)");
            println!("  - hook.failed      (on error)");
            println!("  - hook.timeout     (if hook exceeds timeout)");
            println!();
            println!("Metrics:");
            println!("  - jarvy.tool.requests      (counter)");
            println!("  - jarvy.tool.installs      (counter by status)");
            println!("  - jarvy.tool.unsupported   (counter — fires per `tool.unsupported` event)");
            println!("  - jarvy.install.duration   (histogram in seconds)");
            println!("  - jarvy.setup.duration     (histogram in seconds)");
            println!();
            println!("Privacy: File paths and secrets are redacted before sending.");
        }
    }
}

/// Clear the machine fingerprint from ~/.jarvy/config.toml
fn clear_machine_fingerprint() {
    // Skip when no config exists at all — preserves prior behavior of
    // silently doing nothing when the file is absent.
    let Some(path) = init::global_config_path() else {
        return;
    };
    if !path.exists() {
        return;
    }
    if let Err(e) = init::modify_global_config(|config| {
        config.settings.fingerprint = None;
    }) {
        tracing::warn!(
            event = "telemetry.fingerprint.clear_failed",
            error = %e,
        );
        return;
    }
    tracing::info!(event = "telemetry.fingerprint.cleared");
}

/// Update telemetry configuration in ~/.jarvy/config.toml
pub fn update_telemetry_config(enabled: bool, endpoint: Option<String>) {
    if let Err(e) = init::modify_global_config(|config| {
        config.telemetry.enabled = enabled;
        if let Some(ep) = endpoint.clone() {
            config.telemetry.endpoint = ep;
        }
    }) {
        eprintln!("Failed to update telemetry config: {e}");
        return;
    }
    tracing::info!(
        event = if enabled {
            "telemetry.enabled"
        } else {
            "telemetry.disabled"
        },
        fingerprint_cleared = !enabled,
    );
}
