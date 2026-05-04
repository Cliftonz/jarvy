//! Services command handler - manage project services (docker-compose, tilt)

use crate::ci;
use crate::cli::ServicesAction;
use crate::config::Config;
use crate::services;

/// Run the services command
pub fn run_services(action: &ServicesAction, file: &str) -> i32 {
    let config = Config::new(file);
    let services_config = config.services.clone();

    // Check if services are enabled
    if !services_config.enabled {
        eprintln!("Services are not enabled in the configuration.");
        eprintln!("Add [services] enabled = true to your jarvy.toml");
        return 0;
    }

    // Detect CI environment (available for future auto-start integration)
    let _is_ci = ci::detect().is_some();

    // Get the working directory
    let working_dir = std::path::Path::new(file)
        .parent()
        .unwrap_or(std::path::Path::new("."));

    // Detect service backend (or use config overrides)
    let backend_result = services::detect_backend_with_config(
        working_dir,
        services_config.compose_file.as_deref(),
        services_config.tilt_file.as_deref(),
    );

    let (backend, config_path) = match backend_result {
        Some((b, p)) => (b, p),
        None => {
            eprintln!("No service configuration found.");
            eprintln!("Supported: docker-compose.yml, compose.yml, Tiltfile");
            return 0;
        }
    };

    let backend_impl = services::get_backend(backend);

    // Check if backend is installed.
    //
    // The "not installed" message is informational, not an error — the
    // command returns 0 either way because the user asked us to *report*
    // status, and "tool absent" is a valid status. Print to stdout so
    // status-reading scripts and integration tests can capture it from
    // the same stream as the success path. Without this, the macOS
    // arm64 runners (which don't ship Docker Compose) produced an
    // empty stdout and broke services_detects_docker_compose_yml,
    // services_prioritizes_docker_compose_over_tilt, and
    // services_uses_compose_file_override.
    if !backend_impl.is_installed() {
        println!("{} is not installed.", backend);
        println!("Install it with: jarvy setup");
        return 0;
    }

    match action {
        ServicesAction::Start { foreground } => {
            println!("Starting {} services...", backend);
            let detach = !foreground;
            match backend_impl.start(&config_path, detach) {
                Ok(result) => {
                    println!("{}", result.message);
                }
                Err(e) => {
                    eprintln!("Failed to start services: {}", e);
                    return 1;
                }
            }
        }
        ServicesAction::Stop {} => {
            println!("Stopping {} services...", backend);
            match backend_impl.stop(&config_path) {
                Ok(result) => {
                    println!("{}", result.message);
                }
                Err(e) => {
                    eprintln!("Failed to stop services: {}", e);
                    return 1;
                }
            }
        }
        ServicesAction::Status {} => match backend_impl.status(&config_path) {
            Ok(status) => {
                println!("Service Backend: {}", status.backend);
                println!("Installed: {}", if status.installed { "Yes" } else { "No" });
                println!("Running: {}", if status.running { "Yes" } else { "No" });
                if !status.details.is_empty() {
                    println!("\nDetails:\n{}", status.details);
                }
            }
            Err(e) => {
                eprintln!("Failed to get service status: {}", e);
                return 1;
            }
        },
        ServicesAction::Restart { foreground } => {
            println!("Restarting {} services...", backend);
            let detach = !foreground;
            match backend_impl.restart(&config_path, detach) {
                Ok(result) => {
                    println!("{}", result.message);
                }
                Err(e) => {
                    eprintln!("Failed to restart services: {}", e);
                    return 1;
                }
            }
        }
    }
    0
}
