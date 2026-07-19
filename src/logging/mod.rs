//! Logging module - unified through observability
//!
//! This module re-exports logging utilities from the observability module
//! and provides helper functions for log file management.
//!
//! All logging goes through tracing-subscriber with:
//! - Console output (for interactive use)
//! - File output to ~/.jarvy/logs/ (always enabled)
//! - Optional OTLP export (when telemetry is configured)

use std::path::PathBuf;
use thiserror::Error;

// Re-export from observability
pub use crate::observability::Sanitizer;
pub use crate::observability::logging::{LogConfig, LogFormat, LogLevel};

/// Logging errors
#[derive(Debug, Error)]
pub enum LogError {
    #[error("Failed to create log directory: {0}")]
    DirectoryCreationFailed(#[from] std::io::Error),

    #[error("Failed to open log file: {0}")]
    FileOpenFailed(String),

    #[error("Failed to read log file: {0}")]
    ReadFailed(String),
}

/// Get the default log directory path (~/.jarvy/logs/) via the canonical
/// resolver so a `JARVY_HOME` override is honored.
pub fn default_log_directory() -> PathBuf {
    crate::paths::logs_dir().unwrap_or_else(|_| PathBuf::from(".jarvy/logs"))
}

/// Get the current log file path
pub fn current_log_file() -> PathBuf {
    default_log_directory().join("jarvy.log")
}

/// Read recent log entries from the log file
///
/// Returns the last `lines` entries from the current log file.
pub fn read_recent_logs(lines: usize) -> Result<Vec<String>, LogError> {
    let log_file = current_log_file();

    if !log_file.exists() {
        return Ok(Vec::new());
    }

    let content =
        std::fs::read_to_string(&log_file).map_err(|e| LogError::ReadFailed(e.to_string()))?;

    let all_lines: Vec<&str> = content.lines().collect();
    let start = all_lines.len().saturating_sub(lines);

    Ok(all_lines[start..].iter().map(|s| s.to_string()).collect())
}

/// Get statistics about log files
#[derive(Debug, serde::Serialize)]
pub struct LogStats {
    pub total_files: usize,
    pub total_size_bytes: u64,
    pub current_file_size_bytes: u64,
    pub oldest_entry: Option<String>,
    pub newest_entry: Option<String>,
    pub entries_by_level: std::collections::HashMap<String, usize>,
}

/// Calculate log statistics
pub fn get_log_stats() -> Result<LogStats, LogError> {
    let log_dir = default_log_directory();
    let mut total_files = 0;
    let mut total_size: u64 = 0;
    let mut current_file_size: u64 = 0;

    if log_dir.exists() {
        for entry in (std::fs::read_dir(&log_dir)?).flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(metadata) = path.metadata() {
                    total_files += 1;
                    total_size += metadata.len();
                    if path.file_name().map(|n| n == "jarvy.log").unwrap_or(false) {
                        current_file_size = metadata.len();
                    }
                }
            }
        }
    }

    // Count entries by level from current log
    let mut entries_by_level = std::collections::HashMap::new();
    let mut oldest_entry = None;
    let mut newest_entry = None;

    let log_file = current_log_file();
    if log_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&log_file) {
            let mut newest_line: Option<&str> = None;

            for line in content.lines() {
                if oldest_entry.is_none() {
                    oldest_entry = Some(line.to_string());
                }
                newest_line = Some(line);

                // Try to parse log level from line (works with both JSON and text formats)
                let level = if line.contains("\"level\":\"ERROR\"") || line.contains(" ERROR ") {
                    Some("ERROR")
                } else if line.contains("\"level\":\"WARN\"") || line.contains(" WARN ") {
                    Some("WARN")
                } else if line.contains("\"level\":\"INFO\"") || line.contains(" INFO ") {
                    Some("INFO")
                } else if line.contains("\"level\":\"DEBUG\"") || line.contains(" DEBUG ") {
                    Some("DEBUG")
                } else if line.contains("\"level\":\"TRACE\"") || line.contains(" TRACE ") {
                    Some("TRACE")
                } else {
                    None
                };
                if let Some(level) = level {
                    if let Some(count) = entries_by_level.get_mut(level) {
                        *count += 1;
                    } else {
                        entries_by_level.insert(level.to_string(), 1);
                    }
                }
            }

            newest_entry = newest_line.map(|s| s.to_string());
        }
    }

    Ok(LogStats {
        total_files,
        total_size_bytes: total_size,
        current_file_size_bytes: current_file_size,
        oldest_entry,
        newest_entry,
        entries_by_level,
    })
}

/// Clean old log files
pub fn clean_logs(max_age_days: u32, all: bool) -> Result<(usize, u64), LogError> {
    let log_dir = default_log_directory();

    if !log_dir.exists() {
        return Ok((0, 0));
    }

    let mut removed_files = 0;
    let mut removed_bytes: u64 = 0;
    let max_age_secs = max_age_days as u64 * 24 * 60 * 60;

    for entry in std::fs::read_dir(&log_dir)?.flatten() {
        let path = entry.path();
        if path.is_file() {
            let should_remove = if all {
                true
            } else {
                // Check age
                if let Ok(metadata) = path.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        let age = std::time::SystemTime::now()
                            .duration_since(modified)
                            .unwrap_or_default();
                        age.as_secs() > max_age_secs
                    } else {
                        false
                    }
                } else {
                    false
                }
            };

            if should_remove {
                if let Ok(metadata) = path.metadata() {
                    removed_bytes += metadata.len();
                }
                if std::fs::remove_file(&path).is_ok() {
                    removed_files += 1;
                }
            }
        }
    }

    Ok((removed_files, removed_bytes))
}

/// Per-file result from `strip_log_lines`.
#[derive(Debug, Clone)]
pub struct StripResult {
    pub path: PathBuf,
    pub lines_removed: usize,
    pub bytes_saved: u64,
}

/// Compile a `--filter` pattern into a matcher.
///
/// `event=NAME` matches the JSON-encoded structured field
/// `"event":"NAME"` (whitespace-tolerant), which is the shape jarvy's
/// file appender writes. Anything else falls back to a raw substring
/// match on the line.
fn build_line_matcher(pattern: &str) -> Box<dyn Fn(&str) -> bool + Send + Sync> {
    if let Some(name) = pattern.strip_prefix("event=") {
        let needle_tight = format!("\"event\":\"{}\"", name);
        let needle_spaced = format!("\"event\": \"{}\"", name);
        Box::new(move |line| line.contains(&needle_tight) || line.contains(&needle_spaced))
    } else {
        let needle = pattern.to_string();
        Box::new(move |line| line.contains(&needle))
    }
}

/// Strip lines matching `pattern` from rotated log files.
///
/// Active `jarvy.log` is skipped — it may be held open by other jarvy
/// processes, and writes race with the file appender. Without `all`,
/// only files older than `max_age_days` are touched (intersects the
/// existing retention gate so `--filter` alone doesn't silently
/// rewrite fresh files). With `all`, every rotated file is scanned.
///
/// When `dry_run` is true, no files are written; the returned per-file
/// counts reflect what would be stripped.
pub fn strip_log_lines(
    pattern: &str,
    max_age_days: u32,
    all: bool,
    dry_run: bool,
) -> Result<Vec<StripResult>, LogError> {
    let log_dir = default_log_directory();
    if !log_dir.exists() {
        return Ok(Vec::new());
    }

    let active = current_log_file();
    let max_age_secs = max_age_days as u64 * 24 * 60 * 60;
    let now = std::time::SystemTime::now();
    let matches = build_line_matcher(pattern);

    let mut results: Vec<StripResult> = Vec::new();
    for entry in std::fs::read_dir(&log_dir)?.flatten() {
        let path = entry.path();
        if !path.is_file() || path == active {
            continue;
        }

        if !all {
            let eligible = path
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|mt| now.duration_since(mt).ok())
                .is_some_and(|age| age.as_secs() > max_age_secs);
            if !eligible {
                continue;
            }
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let trailing_newline = content.ends_with('\n');
        let mut kept = String::with_capacity(content.len());
        let mut lines_removed = 0usize;
        let mut bytes_saved: u64 = 0;
        for line in content.lines() {
            if matches(line) {
                lines_removed += 1;
                // +1 for the newline that came with the line
                bytes_saved += line.len() as u64 + 1;
            } else {
                kept.push_str(line);
                kept.push('\n');
            }
        }
        if !trailing_newline && !kept.is_empty() {
            kept.pop();
        }

        if lines_removed == 0 {
            continue;
        }

        if !dry_run {
            let tmp = path.with_extension("stripping");
            std::fs::write(&tmp, kept.as_bytes())?;
            std::fs::rename(&tmp, &path)?;
        }

        results.push(StripResult {
            path,
            lines_removed,
            bytes_saved,
        });
    }

    Ok(results)
}

/// Format bytes as human-readable size
pub fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialized against the `jarvy_home_env` group because
    /// `default_log_directory()` reads JARVY_HOME — concurrent tests
    /// that pin a tempdir into JARVY_HOME otherwise race with the
    /// `.jarvy/logs` suffix assertion.
    #[test]
    fn build_matcher_event_prefix_hits_structured_field() {
        let m = build_line_matcher("event=tools_d_unsafe_perms");
        assert!(m(r#"{"level":"WARN","fields":{"event":"tools_d_unsafe_perms"}}"#));
        assert!(m(r#"{"fields":{"event": "tools_d_unsafe_perms","x":1}}"#));
        // Substring collision in message payload must NOT match.
        assert!(!m(r#"{"fields":{"message":"tools_d_unsafe_perms happened"}}"#));
    }

    #[test]
    fn build_matcher_bare_pattern_is_substring() {
        let m = build_line_matcher("shell_init.generated");
        assert!(m(r#"{"fields":{"event":"shell_init.generated"}}"#));
        assert!(m("prefix shell_init.generated suffix"));
        assert!(!m("nothing to see"));
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn strip_log_lines_removes_matches_and_skips_active() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // SAFETY: single-threaded test group `jarvy_home_env`.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("JARVY_HOME", tmp.path());
        }
        let logs = tmp.path().join("logs");
        std::fs::create_dir_all(&logs).unwrap();

        // Rotated file with mixed lines
        let rotated = logs.join("jarvy.log.2026-01-01");
        std::fs::write(
            &rotated,
            "{\"fields\":{\"event\":\"noise\"}}\n\
             {\"fields\":{\"event\":\"keep_me\"}}\n\
             {\"fields\":{\"event\":\"noise\"}}\n",
        )
        .unwrap();
        // Backdate so the retention gate (default 30 days) treats it as old.
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(60 * 24 * 3600);
        filetime::set_file_mtime(&rotated, filetime::FileTime::from_system_time(old)).unwrap();

        // Active file must NOT be touched even if it contains matches.
        let active = logs.join("jarvy.log");
        std::fs::write(&active, "{\"fields\":{\"event\":\"noise\"}}\n").unwrap();

        let results = strip_log_lines("event=noise", 30, false, false).expect("strip");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].lines_removed, 2);
        assert_eq!(results[0].path, rotated);

        let after = std::fs::read_to_string(&rotated).unwrap();
        assert_eq!(after, "{\"fields\":{\"event\":\"keep_me\"}}\n");
        // Active untouched.
        assert_eq!(
            std::fs::read_to_string(&active).unwrap(),
            "{\"fields\":{\"event\":\"noise\"}}\n"
        );

        // SAFETY: same group.
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("JARVY_HOME");
        }
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn strip_log_lines_dry_run_reports_without_writing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("JARVY_HOME", tmp.path());
        }
        let logs = tmp.path().join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        let rotated = logs.join("jarvy.log.2026-01-02");
        let body = "{\"fields\":{\"event\":\"drop\"}}\n{\"fields\":{\"event\":\"stay\"}}\n";
        std::fs::write(&rotated, body).unwrap();

        // --all bypasses the age gate.
        let results = strip_log_lines("event=drop", 30, true, true).expect("strip");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].lines_removed, 1);
        // File unchanged.
        assert_eq!(std::fs::read_to_string(&rotated).unwrap(), body);

        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("JARVY_HOME");
        }
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn test_default_log_directory() {
        let dir = default_log_directory();
        assert!(dir.ends_with(".jarvy/logs"));
    }

    #[test]
    fn test_current_log_file() {
        let file = current_log_file();
        assert!(file.ends_with("jarvy.log"));
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
    }
}
