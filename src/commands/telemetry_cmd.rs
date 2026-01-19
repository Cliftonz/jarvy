//! Telemetry command handler - manage telemetry settings

use std::fs;

use crate::cli::TelemetryAction;
use crate::init;
use crate::telemetry;

/// Handle telemetry subcommands
pub fn run_telemetry(action: &TelemetryAction, _global_config: &init::CliConfig) {
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
            println!();
            println!("Configuration sources:");
            println!("  - Config file: ~/.jarvy/config.toml [telemetry] section");
            println!("  - Environment: JARVY_TELEMETRY, JARVY_OTLP_ENDPOINT");
        }
        TelemetryAction::Enable {} => {
            update_telemetry_config(true, None);
            println!("Telemetry enabled.");
            println!("Configure endpoint with: jarvy telemetry set-endpoint <url>");
        }
        TelemetryAction::Disable {} => {
            update_telemetry_config(false, None);
            println!("Telemetry disabled.");
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
            println!("  - tool.requested   (per tool in config)");
            println!("  - tool.installed   (for each successful install)");
            println!("  - tool.failed      (for each failed install)");
            println!("  - tool.not_supported (for unknown tools)");
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
            println!("  - jarvy.install.duration   (histogram in seconds)");
            println!("  - jarvy.setup.duration     (histogram in seconds)");
            println!();
            println!("Privacy: File paths and secrets are redacted before sending.");
        }
    }
}

/// Update telemetry configuration in ~/.jarvy/config.toml
pub fn update_telemetry_config(enabled: bool, endpoint: Option<String>) {
    let home_dir = match dirs::home_dir() {
        Some(h) => h,
        None => {
            eprintln!("Could not determine home directory");
            return;
        }
    };
    let config_path = home_dir.join(".jarvy").join("config.toml");

    // Read existing config
    let mut config: init::CliConfig = if config_path.exists() {
        let content = fs::read_to_string(&config_path).unwrap_or_default();
        toml::from_str(&content).unwrap_or_default()
    } else {
        init::CliConfig::default()
    };

    // Update telemetry settings
    config.telemetry.enabled = enabled;
    if let Some(ep) = endpoint {
        config.telemetry.endpoint = ep;
    }

    // Write back
    match toml::to_string_pretty(&config) {
        Ok(content) => {
            if let Err(e) = fs::write(&config_path, content) {
                eprintln!("Failed to write config: {}", e);
            }
        }
        Err(e) => {
            eprintln!("Failed to serialize config: {}", e);
        }
    }
}
