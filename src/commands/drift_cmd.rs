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

/// Handle drift subcommands. When invoked from inside a workspace
/// member (auto-detected via `setup_cmd::auto_detect_project`), drift
/// reads the member's `jarvy.toml` so baseline state lives next to
/// that member's `.jarvy/state.json` instead of the workspace root
/// (PRD-047 phase 2).
pub fn run_drift(file: &str, action: &DriftAction) -> i32 {
    let (file, project_dir) = workspace_aware_path(file);
    let file = file.as_str();

    match action {
        DriftAction::Check { output_format } => run_drift_check(&project_dir, file, output_format),
        DriftAction::Status {
            verbose,
            output_format,
        } => run_drift_status(&project_dir, *verbose, output_format),
        DriftAction::Accept {
            tools,
            output_format,
        } => run_drift_accept(&project_dir, file, tools.as_deref(), output_format),
        DriftAction::Fix {
            dry_run,
            force: _,
            output_format,
        } => run_drift_fix(&project_dir, file, *dry_run, output_format),
    }
}

/// Apply workspace auto-context (PRD-047 phase 2). Returns the
/// effective config file path and the directory drift should treat
/// as the project root (where `.jarvy/state.json` lives). Glue is
/// centralized in `setup_cmd::effective_config_path` so any future
/// changes to the auto-context rule land in one place instead of
/// being replayed across setup / doctor / drift / context.
fn workspace_aware_path(file: &str) -> (String, std::path::PathBuf) {
    let resolved = crate::commands::setup_cmd::effective_config_path(file);
    let dir = resolved.parent().unwrap_or(Path::new(".")).to_path_buf();
    let path = resolved.to_string_lossy().into_owned();
    if path != file && crate::observability::telemetry_gate::is_enabled() {
        // `resolved` (the full config path) is deliberately NOT
        // emitted — the leading `$HOME` on macOS/Windows carries the
        // account name and matches the PII pattern
        // agent_profile.cwd_hint / config.personal_overlay_applied
        // already resolved. `reason` is the diagnostic signal.
        tracing::info!(
            event = "drift.context.auto_redirected",
            reason = "cwd_inside_workspace_member",
        );
    }
    (path, dir)
}

/// Emit a bounded warn event when a `.jarvy/state.json` I/O op
/// fails. Callers still surface the error via `eprintln!` for the
/// user; this adds the persistent trail on-call needs.
fn emit_state_io_failed(op: &'static str, err: &crate::drift::DriftError) {
    if crate::observability::telemetry_gate::is_enabled() {
        tracing::warn!(
            event = "drift.state.io_failed",
            op = op,
            error_kind = err.kind_label(),
            error = %err,
        );
    }
}

/// Run drift check command
fn run_drift_check(project_dir: &Path, config_file: &str, output_format: &str) -> i32 {
    let started = std::time::Instant::now();

    // Load config
    let config = Config::new(config_file);
    let drift_config = config.drift.clone().unwrap_or_default();

    if !drift_config.enabled {
        println!("Drift detection is disabled in configuration.");
        println!("Enable it by setting [drift] enabled = true in jarvy.toml");
        return 0;
    }

    // Load baseline state
    let state = match EnvironmentState::load(project_dir) {
        Ok(Some(state)) => state,
        Ok(None) => {
            println!("\x1b[33m⚠\x1b[0m No baseline state found.");
            println!("  Run 'jarvy setup' to capture the initial state, or");
            println!("  Run 'jarvy drift accept' to create a baseline from current state.");
            emit_drift_check_completed(
                "no_baseline",
                0,
                0,
                0,
                0,
                started.elapsed().as_millis() as u64,
            );
            return 1;
        }
        Err(e) => {
            eprintln!("Failed to load state: {}", e);
            emit_state_io_failed("load", &e);
            return 1;
        }
    };

    // Run drift detection
    let detector = DriftDetector::new(&drift_config, &state, project_dir);
    let report = match detector.detect() {
        Ok(report) => report,
        Err(e) => {
            eprintln!("Drift detection failed: {}", e);
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::warn!(
                    event = "drift.check.failed",
                    error_kind = e.kind_label(),
                    error = %e,
                );
            }
            return 1;
        }
    };

    // Output report
    if output_format == "json" {
        match DriftReporter::to_json(&report) {
            Ok(json) => println!("{}", json),
            Err(e) => {
                eprintln!("Failed to serialize report: {}", e);
                return 1;
            }
        }
    } else {
        DriftReporter::print_report(&report);
    }

    emit_drift_check_completed(
        match report.status {
            DriftStatus::NoDrift => "no_drift",
            DriftStatus::DriftDetected => "drift_detected",
            DriftStatus::NoBaseline => "no_baseline",
        },
        report.summary.version_changes,
        report.summary.missing_tools,
        report.summary.changed_files,
        report.summary.total_issues,
        started.elapsed().as_millis() as u64,
    );

    // Return appropriate code
    match report.status {
        DriftStatus::NoDrift => 0,
        DriftStatus::DriftDetected => 1,
        DriftStatus::NoBaseline => 2,
    }
}

/// Emit the drift.check.completed lifecycle event. Bounded status
/// label + integer counters + duration only — no version strings, no
/// paths (cardinality bombs per analytics F5).
fn emit_drift_check_completed(
    status: &'static str,
    version_change_count: usize,
    missing_count: usize,
    changed_files_count: usize,
    drifted_count: usize,
    duration_ms: u64,
) {
    if crate::observability::telemetry_gate::is_enabled() {
        tracing::info!(
            event = "drift.check.completed",
            status = status,
            drifted_count = drifted_count,
            version_change_count = version_change_count,
            missing_count = missing_count,
            changed_files_count = changed_files_count,
            duration_ms = duration_ms,
        );
    }
}

/// Run drift status command
fn run_drift_status(project_dir: &Path, verbose: bool, output_format: &str) -> i32 {
    let state = match EnvironmentState::load(project_dir) {
        Ok(Some(state)) => state,
        Ok(None) => {
            if output_format == "json" {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "no_baseline",
                        "message": "no baseline state found",
                    })
                );
            } else {
                println!("\x1b[33m⚠\x1b[0m No baseline state found.");
                println!("  The baseline is captured automatically after 'jarvy setup'.");
                println!("  Or run 'jarvy drift accept' to create one manually.");
            }
            return 0;
        }
        Err(e) => {
            if output_format == "json" {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "error",
                        "error": e.to_string(),
                    })
                );
            } else {
                eprintln!("Failed to load state: {}", e);
            }
            emit_state_io_failed("load", &e);
            return 1;
        }
    };

    if output_format == "json" {
        let json = serde_json::json!({
            "status": "ok",
            "state_version": state.version,
            "created_at": state.created_at,
            "updated_at": state.updated_at,
            "tools": state.tools.iter().map(|(name, tool)| {
                serde_json::json!({
                    "name": name,
                    "version": tool.version,
                    "install_method": tool.install_method,
                    "path": tool.path.display().to_string(),
                })
            }).collect::<Vec<_>>(),
            "files": state.files.iter().map(|(path, hash)| {
                serde_json::json!({ "path": path, "hash": hash })
            }).collect::<Vec<_>>(),
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&json).unwrap_or_else(|_| json.to_string())
        );
        return 0;
    }

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

    0
}

/// Run drift accept command
fn run_drift_accept(
    project_dir: &Path,
    config_file: &str,
    tools_filter: Option<&str>,
    output_format: &str,
) -> i32 {
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
        let binary = crate::tools::spec::binary_for(tool_name);
        if let Some(installed) = crate::drift::detector::get_tool_version(binary) {
            let path = which::which(binary).unwrap_or_else(|_| std::path::PathBuf::from("unknown"));

            let install_method = detect_install_method(tool_name);
            // Preserve config constraint separately so a later
            // `jarvy drift check` compares against the concrete
            // installed version, not the constraint (Codex P1 fix).
            let constraint = tool_configs
                .get(tool_name)
                .map(|t| t.version.as_str())
                .unwrap_or(&installed);
            state.set_tool_with_installed(
                tool_name,
                constraint,
                Some(&installed),
                &path,
                &install_method,
            );
            accepted += 1;
        }
    }

    // Update tracked file hashes
    for file_path in &drift_config.track_files {
        let full_path = project_dir.join(file_path);
        if full_path.exists()
            && let Ok(hash) = crate::drift::state::hash_file(&full_path)
        {
            state.set_file_hash(file_path, &hash);
        }
    }

    // Update config hash
    let config_path = project_dir.join("jarvy.toml");
    if config_path.exists()
        && let Ok(hash) = crate::drift::state::hash_file(&config_path)
    {
        state.set_config_hash(&hash);
    }

    // Save state
    if let Err(e) = state.save(project_dir) {
        if output_format == "json" {
            println!(
                "{}",
                serde_json::json!({
                    "status": "error",
                    "error": e.to_string(),
                })
            );
        } else {
            eprintln!("Failed to save state: {}", e);
        }
        emit_state_io_failed("save", &e);
        return 1;
    }

    if crate::observability::telemetry_gate::is_enabled() {
        tracing::info!(
            event = "drift.baseline.accepted",
            tools_accepted = accepted,
            files_tracked = drift_config.track_files.len(),
            source = "manual_accept",
        );
    }

    if output_format == "json" {
        let json = serde_json::json!({
            "status": "ok",
            "tools_accepted": accepted,
            "files_tracked": drift_config.track_files.len(),
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&json).unwrap_or_else(|_| json.to_string())
        );
    } else {
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

    0
}

/// Run drift fix command
fn run_drift_fix(project_dir: &Path, config_file: &str, dry_run: bool, output_format: &str) -> i32 {
    // Load config
    let config = Config::new(config_file);
    let drift_config = config.drift.clone().unwrap_or_default();

    if !drift_config.enabled {
        if output_format == "json" {
            println!(
                "{}",
                serde_json::json!({"status": "disabled", "message": "drift detection disabled"})
            );
        } else {
            println!("Drift detection is disabled in configuration.");
        }
        return 0;
    }

    // Load baseline state
    let state = match EnvironmentState::load(project_dir) {
        Ok(Some(state)) => state,
        Ok(None) => {
            if output_format == "json" {
                println!(
                    "{}",
                    serde_json::json!({"status": "no_baseline", "message": "no baseline state found"})
                );
            } else {
                println!("\x1b[33m⚠\x1b[0m No baseline state found.");
                println!("  Run 'jarvy setup' first to establish a baseline.");
            }
            return 1;
        }
        Err(e) => {
            if output_format == "json" {
                println!(
                    "{}",
                    serde_json::json!({"status": "error", "error": e.to_string()})
                );
            } else {
                eprintln!("Failed to load state: {}", e);
            }
            emit_state_io_failed("load", &e);
            return 1;
        }
    };

    let started = std::time::Instant::now();

    // Detect drift
    let detector = DriftDetector::new(&drift_config, &state, project_dir);
    let report = match detector.detect() {
        Ok(report) => report,
        Err(e) => {
            if output_format == "json" {
                println!(
                    "{}",
                    serde_json::json!({"status": "error", "error": e.to_string()})
                );
            } else {
                eprintln!("Drift detection failed: {}", e);
            }
            if crate::observability::telemetry_gate::is_enabled() {
                tracing::warn!(
                    event = "drift.fix.failed",
                    error_kind = e.kind_label(),
                    error = %e,
                );
            }
            return 1;
        }
    };

    if report.status == DriftStatus::NoDrift {
        if output_format == "json" {
            println!(
                "{}",
                serde_json::json!({"status": "no_drift", "dry_run": dry_run})
            );
        } else {
            println!("\x1b[32m✓\x1b[0m No drift detected, nothing to fix.");
        }
        return 0;
    }

    if dry_run && output_format != "json" {
        println!("\x1b[36mDry run mode\x1b[0m - no changes will be made\n");
    }

    // Run fixer
    let fixer = DriftFixer::new(dry_run);
    let results = fixer.fix_all(&report);

    // Aggregate lifecycle telemetry (item 3 / F2). Per-tool events
    // land as `drift.fix.tool_result` — labels are the tool name,
    // registry-derived category, and bounded outcome/kind. NEVER
    // expected_version / actual_version / path as labels (cardinality
    // bomb — analytics F5).
    let mut succeeded = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut dry_run_count = 0;
    for r in &results {
        let outcome = match r.status {
            crate::drift::FixStatus::Success => {
                succeeded += 1;
                "success"
            }
            crate::drift::FixStatus::Failed => {
                failed += 1;
                "failed"
            }
            crate::drift::FixStatus::Skipped => {
                skipped += 1;
                "skipped"
            }
            crate::drift::FixStatus::DryRun => {
                dry_run_count += 1;
                "dry_run"
            }
        };
        if crate::observability::telemetry_gate::is_enabled() {
            let category =
                crate::tools::spec::get_tool_category(&r.target).unwrap_or("uncategorized");
            tracing::info!(
                event = "drift.fix.tool_result",
                tool = %r.target,
                category = category,
                outcome = outcome,
            );
        }
    }
    if crate::observability::telemetry_gate::is_enabled() {
        tracing::info!(
            event = "drift.fix.completed",
            dry_run = dry_run,
            attempted = results.len(),
            succeeded = succeeded,
            failed = failed,
            skipped = skipped,
            dry_run_count = dry_run_count,
            duration_ms = started.elapsed().as_millis() as u64,
        );
    }

    if output_format == "json" {
        let json = serde_json::json!({
            "status": "fixed",
            "dry_run": dry_run,
            "results": results,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&json).unwrap_or_else(|_| json.to_string())
        );
    } else {
        DriftFixer::print_summary(&results);
    }

    0
}

/// Detect install method for a tool. Delegates to the canonical
/// classifier in `tools::install_method`. drift writes this string
/// into `state.json`, so the canonical labels are wire-format here
/// (round-2 maint F1).
fn detect_install_method(tool: &str) -> String {
    crate::tools::install_method::detect_install_method_for_tool(tool).to_string()
}
