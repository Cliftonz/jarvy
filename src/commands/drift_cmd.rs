//! Drift detection command handler
//!
//! Handles the `jarvy drift` subcommands:
//! - `check` - Detect configuration drift
//! - `status` - Show current baseline state
//! - `accept` - Accept current state as new baseline
//! - `fix` - Remediate detected drift

use std::path::Path;

use crate::cli::DriftAction;
use crate::config::Config;
use crate::drift::{DriftDetector, DriftFixer, DriftReporter, DriftStatus, EnvironmentState};

/// Handle drift subcommands
pub fn run_drift(file: &str, action: &DriftAction) {
    let project_dir = Path::new(file)
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    match action {
        DriftAction::Check { output_format } => {
            run_drift_check(&project_dir, file, output_format);
        }
        DriftAction::Status { verbose } => {
            run_drift_status(&project_dir, *verbose);
        }
        DriftAction::Accept { tools } => {
            run_drift_accept(&project_dir, file, tools.as_deref());
        }
        DriftAction::Fix { dry_run, force: _ } => {
            run_drift_fix(&project_dir, file, *dry_run);
        }
    }
}

/// Run drift check command
fn run_drift_check(project_dir: &Path, config_file: &str, output_format: &str) {
    // Load config
    let config = Config::new(config_file);
    let drift_config = config.drift.clone().unwrap_or_default();

    if !drift_config.enabled {
        println!("Drift detection is disabled in configuration.");
        println!("Enable it by setting [drift] enabled = true in jarvy.toml");
        return;
    }

    // Load baseline state
    let state = match EnvironmentState::load(project_dir) {
        Ok(Some(state)) => state,
        Ok(None) => {
            println!("\x1b[33m⚠\x1b[0m No baseline state found.");
            println!("  Run 'jarvy setup' to capture the initial state, or");
            println!("  Run 'jarvy drift accept' to create a baseline from current state.");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Failed to load state: {}", e);
            std::process::exit(1);
        }
    };

    // Run drift detection
    let detector = DriftDetector::new(&drift_config, &state, project_dir);
    let report = match detector.detect() {
        Ok(report) => report,
        Err(e) => {
            eprintln!("Drift detection failed: {}", e);
            std::process::exit(1);
        }
    };

    // Output report
    if output_format == "json" {
        match DriftReporter::to_json(&report) {
            Ok(json) => println!("{}", json),
            Err(e) => {
                eprintln!("Failed to serialize report: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        DriftReporter::print_report(&report);
    }

    // Exit with appropriate code
    match report.status {
        DriftStatus::NoDrift => std::process::exit(0),
        DriftStatus::DriftDetected => std::process::exit(1),
        DriftStatus::NoBaseline => std::process::exit(2),
    }
}

/// Run drift status command
fn run_drift_status(project_dir: &Path, verbose: bool) {
    let state = match EnvironmentState::load(project_dir) {
        Ok(Some(state)) => state,
        Ok(None) => {
            println!("\x1b[33m⚠\x1b[0m No baseline state found.");
            println!("  The baseline is captured automatically after 'jarvy setup'.");
            println!("  Or run 'jarvy drift accept' to create one manually.");
            return;
        }
        Err(e) => {
            eprintln!("Failed to load state: {}", e);
            std::process::exit(1);
        }
    };

    println!("\x1b[1mDrift Detection Baseline\x1b[0m");
    println!("========================");
    println!("State version: {}", state.version);
    println!("Created: {}", state.created_at);
    println!("Updated: {}", state.updated_at);
    println!();

    println!("\x1b[1mTracked Tools ({}):\x1b[0m", state.tools.len());
    for (name, tool) in &state.tools {
        if verbose {
            println!(
                "  {} {} (via {}, at {})",
                name,
                tool.version,
                tool.install_method,
                tool.path.display()
            );
        } else {
            println!("  {} {}", name, tool.version);
        }
    }

    if !state.files.is_empty() {
        println!();
        println!("\x1b[1mTracked Files ({}):\x1b[0m", state.files.len());
        for (path, hash) in &state.files {
            if verbose {
                println!("  {} ({})", path, hash);
            } else {
                println!("  {}", path);
            }
        }
    }
}

/// Run drift accept command
fn run_drift_accept(project_dir: &Path, config_file: &str, tools_filter: Option<&str>) {
    // Load config to get tracked files
    let config = Config::new(config_file);
    let drift_config = config.drift.clone().unwrap_or_default();

    // Load existing state or create new
    let mut state = EnvironmentState::load(project_dir)
        .ok()
        .flatten()
        .unwrap_or_default();

    // Get current tool states
    let tool_configs = config.get_tool_configs();
    let tools_to_accept: Vec<String> = if let Some(filter) = tools_filter {
        filter.split(',').map(|s| s.trim().to_string()).collect()
    } else {
        // Accept all tools from config
        tool_configs.keys().cloned().collect()
    };

    let mut accepted = 0;
    for tool_name in &tools_to_accept {
        if let Some(version) = get_installed_version(tool_name) {
            let path = which::which(tool_name.as_str())
                .unwrap_or_else(|_| std::path::PathBuf::from("unknown"));

            // Determine install method (simplified detection)
            let install_method = detect_install_method(tool_name);

            state.set_tool(tool_name, &version, &path, &install_method);
            accepted += 1;
        }
    }

    // Update tracked file hashes
    for file_path in &drift_config.track_files {
        let full_path = project_dir.join(file_path);
        if full_path.exists() {
            if let Ok(hash) = crate::drift::state::hash_file(&full_path) {
                state.set_file_hash(file_path, &hash);
            }
        }
    }

    // Update config hash
    let config_path = project_dir.join("jarvy.toml");
    if config_path.exists() {
        if let Ok(hash) = crate::drift::state::hash_file(&config_path) {
            state.set_config_hash(&hash);
        }
    }

    // Save state
    if let Err(e) = state.save(project_dir) {
        eprintln!("Failed to save state: {}", e);
        std::process::exit(1);
    }

    println!("\x1b[32m✓\x1b[0m Baseline state updated");
    println!(
        "  {} tool{} accepted",
        accepted,
        if accepted == 1 { "" } else { "s" }
    );
    if !drift_config.track_files.is_empty() {
        println!(
            "  {} file{} tracked",
            drift_config.track_files.len(),
            if drift_config.track_files.len() == 1 {
                ""
            } else {
                "s"
            }
        );
    }
}

/// Run drift fix command
fn run_drift_fix(project_dir: &Path, config_file: &str, dry_run: bool) {
    // Load config
    let config = Config::new(config_file);
    let drift_config = config.drift.clone().unwrap_or_default();

    if !drift_config.enabled {
        println!("Drift detection is disabled in configuration.");
        return;
    }

    // Load baseline state
    let state = match EnvironmentState::load(project_dir) {
        Ok(Some(state)) => state,
        Ok(None) => {
            println!("\x1b[33m⚠\x1b[0m No baseline state found.");
            println!("  Run 'jarvy setup' first to establish a baseline.");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Failed to load state: {}", e);
            std::process::exit(1);
        }
    };

    // Detect drift
    let detector = DriftDetector::new(&drift_config, &state, project_dir);
    let report = match detector.detect() {
        Ok(report) => report,
        Err(e) => {
            eprintln!("Drift detection failed: {}", e);
            std::process::exit(1);
        }
    };

    if report.status == DriftStatus::NoDrift {
        println!("\x1b[32m✓\x1b[0m No drift detected, nothing to fix.");
        return;
    }

    if dry_run {
        println!("\x1b[36mDry run mode\x1b[0m - no changes will be made\n");
    }

    // Run fixer
    let fixer = DriftFixer::new(dry_run);
    let results = fixer.fix_all(&report);

    DriftFixer::print_summary(&results);
}

/// Get installed version of a tool
fn get_installed_version(tool: &str) -> Option<String> {
    use std::process::Command;

    let output = Command::new(tool)
        .arg("--version")
        .output()
        .or_else(|_| Command::new(tool).arg("-V").output())
        .or_else(|_| Command::new(tool).arg("version").output())
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    extract_version(&output_str)
}

/// Extract version from command output
fn extract_version(output: &str) -> Option<String> {
    let version_regex =
        regex::Regex::new(r"(?i)v?(\d+\.\d+(?:\.\d+)?(?:-[a-zA-Z0-9.]+)?(?:\+[a-zA-Z0-9.]+)?)")
            .ok()?;

    version_regex
        .captures(output)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// Detect install method for a tool
fn detect_install_method(tool: &str) -> String {
    // Check common locations
    if let Ok(path) = which::which(tool) {
        let path_str = path.to_string_lossy();

        if path_str.contains("/homebrew/") || path_str.contains("/opt/homebrew/") {
            return "brew".to_string();
        }
        if path_str.contains("/.cargo/") {
            return "cargo".to_string();
        }
        if path_str.contains("/.nvm/") {
            return "nvm".to_string();
        }
        if path_str.contains("/.pyenv/") {
            return "pyenv".to_string();
        }
        if path_str.contains("/.rustup/") {
            return "rustup".to_string();
        }
        if path_str.contains("/usr/bin/") || path_str.contains("/usr/local/bin/") {
            return "system".to_string();
        }
    }

    "unknown".to_string()
}
