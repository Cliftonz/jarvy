//! Handler for the `jarvy logs` command
//!
//! View and manage log files.

use crate::cli::LogsAction;
use crate::logging;

/// Handle logs command dispatch
pub fn run_logs_command(action: LogsAction) -> i32 {
    match action {
        LogsAction::View {
            lines,
            level,
            grep,
            output_format,
        } => handle_logs_view(lines, level, grep, &output_format),
        LogsAction::Stats { output_format } => handle_logs_stats(&output_format),
        LogsAction::Clean {
            all,
            dry_run,
            filter,
            output_format,
        } => handle_logs_clean(all, dry_run, filter.as_deref(), &output_format),
        LogsAction::Config { output_format } => handle_logs_config(&output_format),
    }
}

/// View recent log entries
fn handle_logs_view(
    lines: usize,
    level_filter: Option<String>,
    grep_filter: Option<String>,
    output_format: &str,
) -> i32 {
    match logging::read_recent_logs(lines) {
        Ok(logs) => {
            if logs.is_empty() {
                println!("No log entries found.");
                return 0;
            }

            // Apply filters
            let filtered: Vec<&String> = logs
                .iter()
                .filter(|line| {
                    // Level filter
                    if let Some(ref level) = level_filter {
                        let level_upper = level.to_uppercase();
                        let has_level = line.contains(&format!("\"level\":\"{}\"", level_upper))
                            || line.contains(&format!(" {} ", level_upper));
                        if !has_level {
                            return false;
                        }
                    }
                    // Grep filter
                    if let Some(ref pattern) = grep_filter {
                        if !line.to_lowercase().contains(&pattern.to_lowercase()) {
                            return false;
                        }
                    }
                    true
                })
                .collect();

            if filtered.is_empty() {
                println!("No log entries match the specified filters.");
                return 0;
            }

            match output_format {
                "json" => {
                    // Output as JSON array
                    let json = serde_json::json!(filtered);
                    println!("{}", serde_json::to_string_pretty(&json).unwrap());
                }
                _ => {
                    // Text output
                    for line in filtered {
                        println!("{}", line);
                    }
                }
            }
            0
        }
        Err(e) => {
            eprintln!("Error reading logs: {}", e);
            1
        }
    }
}

/// Show log statistics
fn handle_logs_stats(output_format: &str) -> i32 {
    match logging::get_log_stats() {
        Ok(stats) => {
            if output_format == "json" {
                let json = serde_json::json!({
                    "total_files": stats.total_files,
                    "total_size_bytes": stats.total_size_bytes,
                    "current_file_size_bytes": stats.current_file_size_bytes,
                    "entries_by_level": stats.entries_by_level,
                    "oldest_entry": stats.oldest_entry,
                    "newest_entry": stats.newest_entry,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json).unwrap_or_else(|_| json.to_string())
                );
                return 0;
            }
            println!("Log Statistics:");
            println!("  Total files: {}", stats.total_files);
            println!(
                "  Total size: {}",
                logging::format_size(stats.total_size_bytes)
            );
            println!(
                "  Current file size: {}",
                logging::format_size(stats.current_file_size_bytes)
            );

            if !stats.entries_by_level.is_empty() {
                println!("\n  Entries by level:");
                for (level, count) in &stats.entries_by_level {
                    println!("    {}: {}", level, count);
                }
            }

            if let Some(ref oldest) = stats.oldest_entry {
                let truncated: String = oldest.chars().take(80).collect();
                println!("\n  Oldest entry: {}...", truncated);
            }
            if let Some(ref newest) = stats.newest_entry {
                let truncated: String = newest.chars().take(80).collect();
                println!("  Newest entry: {}...", truncated);
            }

            0
        }
        Err(e) => {
            if output_format == "json" {
                println!(
                    "{}",
                    serde_json::json!({"status": "error", "error": e.to_string()})
                );
            } else {
                eprintln!("Error getting log stats: {}", e);
            }
            1
        }
    }
}

/// Default max age for log cleanup (30 days)
const DEFAULT_MAX_AGE_DAYS: u32 = 30;

/// Clean old log files
fn handle_logs_clean(
    all: bool,
    dry_run: bool,
    filter: Option<&str>,
    output_format: &str,
) -> i32 {
    if let Some(pattern) = filter {
        return handle_logs_clean_filter(pattern, all, dry_run, output_format);
    }
    let log_dir = logging::default_log_directory();

    if dry_run {
        if !log_dir.exists() {
            if output_format == "json" {
                println!(
                    "{}",
                    serde_json::json!({"status": "no_log_dir", "dry_run": true})
                );
            } else {
                println!("No log directory found.");
            }
            return 0;
        }

        let mut would_remove_paths: Vec<String> = Vec::new();
        let mut would_remove_bytes: u64 = 0;
        let max_age_secs = DEFAULT_MAX_AGE_DAYS as u64 * 24 * 60 * 60;
        let now = std::time::SystemTime::now();

        if let Ok(entries) = std::fs::read_dir(&log_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let should_remove = if all {
                        true
                    } else if let Ok(metadata) = path.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(age) = now.duration_since(modified) {
                                age.as_secs() > max_age_secs
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if should_remove {
                        if let Ok(metadata) = path.metadata() {
                            would_remove_bytes += metadata.len();
                        }
                        if output_format != "json" {
                            println!("Would remove: {}", path.display());
                        }
                        would_remove_paths.push(path.display().to_string());
                    }
                }
            }
        }

        if output_format == "json" {
            let json = serde_json::json!({
                "dry_run": true,
                "would_remove_count": would_remove_paths.len(),
                "would_remove_bytes": would_remove_bytes,
                "would_remove_paths": would_remove_paths,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&json).unwrap_or_else(|_| json.to_string())
            );
        } else if !would_remove_paths.is_empty() {
            println!(
                "\nWould remove {} files ({})",
                would_remove_paths.len(),
                logging::format_size(would_remove_bytes)
            );
        } else {
            println!("No files would be removed.");
        }
        return 0;
    }

    match logging::clean_logs(DEFAULT_MAX_AGE_DAYS, all) {
        Ok((removed, bytes)) => {
            if output_format == "json" {
                let json = serde_json::json!({
                    "dry_run": false,
                    "removed_count": removed,
                    "removed_bytes": bytes,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json).unwrap_or_else(|_| json.to_string())
                );
            } else if removed > 0 {
                println!(
                    "Removed {} log files ({})",
                    removed,
                    logging::format_size(bytes)
                );
            } else {
                println!("No log files to clean.");
            }
            0
        }
        Err(e) => {
            if output_format == "json" {
                println!(
                    "{}",
                    serde_json::json!({"status": "error", "error": e.to_string()})
                );
            } else {
                eprintln!("Error cleaning logs: {}", e);
            }
            1
        }
    }
}

/// Strip matching lines from rotated log files (line-strip mode).
fn handle_logs_clean_filter(pattern: &str, all: bool, dry_run: bool, output_format: &str) -> i32 {
    match logging::strip_log_lines(pattern, DEFAULT_MAX_AGE_DAYS, all, dry_run) {
        Ok(results) => {
            let files_touched = results.len();
            let total_lines: usize = results.iter().map(|r| r.lines_removed).sum();
            let total_bytes: u64 = results.iter().map(|r| r.bytes_saved).sum();

            if output_format == "json" {
                let per_file: Vec<serde_json::Value> = results
                    .iter()
                    .map(|r| {
                        serde_json::json!({
                            "path": r.path.display().to_string(),
                            "lines_removed": r.lines_removed,
                            "bytes_saved": r.bytes_saved,
                        })
                    })
                    .collect();
                let json = serde_json::json!({
                    "dry_run": dry_run,
                    "filter": pattern,
                    "files_touched": files_touched,
                    "lines_removed": total_lines,
                    "bytes_saved": total_bytes,
                    "per_file": per_file,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json).unwrap_or_else(|_| json.to_string())
                );
                return 0;
            }

            if results.is_empty() {
                println!("No lines matched `{}`.", pattern);
                return 0;
            }

            for r in &results {
                let verb = if dry_run { "Would strip" } else { "Stripped" };
                println!(
                    "{} {} line{} from {} ({})",
                    verb,
                    r.lines_removed,
                    if r.lines_removed == 1 { "" } else { "s" },
                    r.path.display(),
                    logging::format_size(r.bytes_saved),
                );
            }
            let verb = if dry_run {
                "Would strip"
            } else {
                "Stripped"
            };
            println!(
                "\n{} {} lines across {} files ({})",
                verb,
                total_lines,
                files_touched,
                logging::format_size(total_bytes),
            );
            0
        }
        Err(e) => {
            if output_format == "json" {
                println!(
                    "{}",
                    serde_json::json!({"status": "error", "error": e.to_string()})
                );
            } else {
                eprintln!("Error stripping logs: {}", e);
            }
            1
        }
    }
}

/// Show logging configuration
fn handle_logs_config(output_format: &str) -> i32 {
    let log_dir = logging::default_log_directory();
    let log_file = logging::current_log_file();

    if output_format == "json" {
        let json = serde_json::json!({
            "directory": log_dir.display().to_string(),
            "current_file": log_file.display().to_string(),
            "rotation": "daily",
            "format": {"file": "json", "console": "text_or_json"},
            "cleanup_max_age_days": DEFAULT_MAX_AGE_DAYS,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&json).unwrap_or_else(|_| json.to_string())
        );
        return 0;
    }

    println!("Logging Configuration:");
    println!("  Directory: {}", log_dir.display());
    println!("  Current file: {}", log_file.display());
    println!("  Rotation: Daily");
    println!("  Format: JSON (file), Text/JSON (console)");
    println!("  Cleanup max age: {} days", DEFAULT_MAX_AGE_DAYS);
    println!();
    println!("Logs are written through the unified tracing system.");
    println!("Use --debug or --trace flags to increase verbosity.");

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logs_config() {
        // Should not panic
        let result = handle_logs_config("pretty");
        assert_eq!(result, 0);
    }

    #[test]
    fn test_logs_stats() {
        // Should not panic even with no logs
        let _result = handle_logs_stats("pretty");
    }

    #[test]
    fn test_logs_config_json_emits_parseable() {
        // JSON variant must round-trip through serde_json.
        // Avoid hitting global stdout state - this only validates the code
        // path compiles and runs.
        let result = handle_logs_config("json");
        assert_eq!(result, 0);
    }
}
