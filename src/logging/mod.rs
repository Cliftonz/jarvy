//! Logging module - unified through observability
//!
//! This module re-exports logging utilities from the observability module
//! and provides helper functions for log file management.
//!
//! All logging goes through tracing-subscriber with:
//! - Console output (for interactive use)
//! - File output to ~/.jarvy/logs/ (always enabled)
//! - Optional OTLP export (when telemetry is configured)

use std::path::{Path, PathBuf};
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
#[derive(Debug, Clone, serde::Serialize)]
pub struct StripResult {
    pub path: PathBuf,
    pub lines_removed: usize,
    pub bytes_saved: u64,
}

/// Aggregate report from a strip run — includes touched files plus
/// counts of files skipped for auditable reasons (perm denied, active
/// log, symlink refused, etc.).
#[derive(Debug, Clone, Default)]
pub struct StripReport {
    pub results: Vec<StripResult>,
    pub skipped_symlink: usize,
    pub skipped_read_failed: usize,
    pub skipped_active: usize,
    pub skipped_too_large: usize,
    pub forensic_refused: Vec<String>,
}

/// High-value refusal events. Stripping them from rotated logs would
/// destroy forensic evidence of security decisions. `--filter event=X`
/// matching any of these is refused unless
/// `--allow-forensic-strip` is passed. Matches the per-domain refusal
/// events in the CLAUDE.md taxonomy.
const FORENSIC_EVENT_NAMES: &[&str] = &[
    "git_config.exec_key_refused",
    "git_config.protect_downgrade_refused",
    "git_config.shell_escape_refused",
    "git_config.shell_alias_refused",
    "git_config.exec_value_refused",
    "mcp.mutation.wizard_bypass_unexpected_client",
    "library.companion.sha_mismatch",
    "library.git.symlink_skipped",
    "library.git.path_escape_refused",
    "library.file_url_refused",
    "wizard.session_token_perms_unsafe",
    "discover.sensitive_key_refused",
];

/// Cap on a single rotated file's read size — a symlink chain, planted
/// huge log, or `/dev/zero`-target would otherwise OOM the process.
/// 512 MiB is generous for real jarvy logs; anything larger deserves
/// human attention rather than silent processing.
const STRIP_MAX_READ_BYTES: u64 = 512 * 1024 * 1024;

/// Compiled matcher for a `--filter` pattern.
///
/// Enum + `#[inline]` dispatch lets the hot per-line loop monomorphize
/// (and inline the underlying `str::contains` SIMD memmem path) —
/// versus the older `Box<dyn Fn>` which paid a vtable indirection per
/// line across MB of log content.
#[derive(Debug, Clone)]
pub enum LineMatcher {
    /// `event=NAME` — structured JSON field match, whitespace-tolerant.
    Event { tight: String, spaced: String },
    /// Bare substring match.
    Substring(String),
}

/// Classification of a `--filter` pattern for low-cardinality telemetry
/// and refusal-list checks. NEVER emit the pattern text itself — users
/// author these, so PII / cross-tenant leakage is a real risk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternKind {
    Event,
    Substring,
}

impl LineMatcher {
    #[inline]
    pub fn matches(&self, line: &str) -> bool {
        match self {
            Self::Event { tight, spaced } => line.contains(tight) || line.contains(spaced),
            Self::Substring(s) => line.contains(s),
        }
    }

    pub fn kind(&self) -> PatternKind {
        match self {
            Self::Event { .. } => PatternKind::Event,
            Self::Substring(_) => PatternKind::Substring,
        }
    }

    /// The `event=<name>` name if this is an Event matcher, for the
    /// forensic-refuse check. `None` for substring.
    pub fn event_name(&self) -> Option<&str> {
        match self {
            Self::Event { tight, .. } => tight
                .strip_prefix("\"event\":\"")
                .and_then(|s| s.strip_suffix('"')),
            Self::Substring(_) => None,
        }
    }
}

/// Compile a `--filter` pattern into a matcher.
///
/// `event=NAME` matches the JSON-encoded structured field
/// `"event":"NAME"` (whitespace-tolerant), which is the shape jarvy's
/// file appender writes. Anything else falls back to a raw substring
/// match on the line.
///
/// Rejects `event=` with empty name (silent no-op typo trap).
pub fn build_line_matcher(pattern: &str) -> Result<LineMatcher, LogError> {
    if let Some(name) = pattern.strip_prefix("event=") {
        if name.is_empty() {
            return Err(LogError::ReadFailed(
                "empty event name after `event=` — expected `event=<name>`".into(),
            ));
        }
        Ok(LineMatcher::Event {
            tight: format!("\"event\":\"{}\"", name),
            spaced: format!("\"event\": \"{}\"", name),
        })
    } else {
        Ok(LineMatcher::Substring(pattern.to_string()))
    }
}

/// Compare two paths as the SAME underlying file via (dev, inode).
///
/// Path-string equality is defeatable by symlink or hardlink — this
/// check is authoritative on Unix. Falls back to string equality on
/// non-Unix targets where `MetadataExt` isn't available.
#[cfg(unix)]
fn same_file(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(ma), Ok(mb)) => ma.dev() == mb.dev() && ma.ino() == mb.ino(),
        _ => a == b,
    }
}

#[cfg(not(unix))]
fn same_file(a: &Path, b: &Path) -> bool {
    a == b
}

/// Strip lines matching `pattern` from rotated log files.
///
/// Active `jarvy.log` is skipped by (dev, inode) — hardlinks and
/// symlinks can't defeat the guard. Symlinks in the log dir are
/// refused entirely to prevent an attacker (co-tenant) from steering
/// the rewrite at an arbitrary file. Rewrite uses an unpredictable
/// tmp name (auto-unlinked on drop) with 0600 mode, and preserves the
/// original file's mode on rename.
///
/// Without `all`, only files older than `max_age_days` are touched.
/// With `all`, every rotated file is scanned.
///
/// `allow_forensic_strip = false` refuses to strip lines carrying
/// event names in [`FORENSIC_EVENT_NAMES`] — those are audit trail
/// for security refusals and shouldn't vanish silently.
///
/// When `dry_run` is true, no files are written.
pub fn strip_log_lines(
    pattern: &str,
    max_age_days: u32,
    all: bool,
    dry_run: bool,
    allow_forensic_strip: bool,
) -> Result<StripReport, LogError> {
    let mut report = StripReport::default();
    let matcher = build_line_matcher(pattern)?;

    // Forensic-refuse check happens before any file touch.
    if !allow_forensic_strip
        && let Some(name) = matcher.event_name()
        && FORENSIC_EVENT_NAMES.contains(&name)
    {
        report.forensic_refused.push(name.to_string());
        return Ok(report);
    }

    let log_dir = default_log_directory();
    if !log_dir.exists() {
        return Ok(report);
    }

    let active = current_log_file();
    let max_age_secs = max_age_days as u64 * 24 * 60 * 60;
    let now = std::time::SystemTime::now();

    for entry in std::fs::read_dir(&log_dir)?.flatten() {
        let path = entry.path();

        // symlink_metadata (not metadata) — reject the entry BEFORE
        // any follow-through operation. Otherwise a co-tenant could
        // plant `jarvy.log.evil -> /etc/passwd` and `read_to_string`
        // + `rename` would overwrite the target.
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.file_type().is_symlink() {
            report.skipped_symlink += 1;
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        if same_file(&path, &active) {
            report.skipped_active += 1;
            continue;
        }

        if !all {
            let eligible = meta
                .modified()
                .ok()
                .and_then(|mt| now.duration_since(mt).ok())
                .is_some_and(|age| age.as_secs() > max_age_secs);
            if !eligible {
                continue;
            }
        }

        // Bounded read — refuse anything absurdly large. Real rotated
        // logs top out at tens of MB; 512 MiB is a soft cap that only
        // catches misconfiguration or attack payloads.
        if meta.len() > STRIP_MAX_READ_BYTES {
            report.skipped_too_large += 1;
            continue;
        }

        let content = match read_file_bounded(&path, meta.len()) {
            Ok(c) => c,
            Err(_) => {
                report.skipped_read_failed += 1;
                continue;
            }
        };

        // Pre-scan: skip allocating `kept` when nothing matches.
        // Rotated logs mostly miss any given filter — this pre-scan
        // eliminates the write-then-discard for the common case.
        if !content.lines().any(|l| matcher.matches(l)) {
            continue;
        }

        let trailing_newline = content.ends_with('\n');
        let mut kept = String::with_capacity(content.len());
        let mut lines_removed = 0usize;
        let mut bytes_saved: u64 = 0;
        for line in content.lines() {
            if matcher.matches(line) {
                lines_removed += 1;
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
            continue; // pre-scan hit but no lines removed — defensive
        }

        if !dry_run {
            write_atomic_preserving_mode(&path, &meta, kept.as_bytes())?;
        }

        report.results.push(StripResult {
            path,
            lines_removed,
            bytes_saved,
        });
    }

    Ok(report)
}

/// Read a file with an up-front capacity hint from its known size —
/// avoids the ~20 realloc chain that `read_to_string` performs on
/// multi-MB files.
fn read_file_bounded(path: &Path, size: u64) -> std::io::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut buf = String::with_capacity(size as usize + 1);
    f.read_to_string(&mut buf)?;
    Ok(buf)
}

/// Atomic rewrite with mode preservation.
///
/// Uses `tempfile::NamedTempFile::new_in` — unpredictable name (no
/// `.stripping` collision), O_EXCL create, auto-unlink on drop if we
/// fail before persist. On Unix, chmod the tmp to match the original
/// mode (default `fs::write` uses `0666 & !umask` — usually `0644`,
/// which would silently downgrade a chmod-600 log to world-readable).
fn write_atomic_preserving_mode(
    path: &Path,
    original_meta: &std::fs::Metadata,
    contents: &[u8],
) -> Result<(), LogError> {
    use std::io::Write;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(contents)?;
    tmp.flush()?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = original_meta.permissions().mode() & 0o7777;
        std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    {
        let _ = original_meta; // silence unused on non-unix
    }

    tmp.persist(path)
        .map_err(|e| LogError::ReadFailed(format!("rename failed: {}", e.error)))?;
    Ok(())
}

/// Sweep stale `.stripping.*` tmp files in the log dir. Kept for
/// backward-compat cleanup — pre-`NamedTempFile` runs that were
/// killed left predictable `.stripping` siblings behind, and any
/// future test that force-kills mid-run could leave a `.tmp*` from
/// tempfile itself. Called at strip startup.
pub fn sweep_stale_strip_tmp() {
    let log_dir = default_log_directory();
    let Ok(entries) = std::fs::read_dir(&log_dir) else {
        return;
    };
    let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let is_stale_tmp = name_str.ends_with(".stripping")
            || (name_str.starts_with(".tmp") && name_str.len() > 4);
        if !is_stale_tmp {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let stale = meta
            .modified()
            .ok()
            .is_some_and(|mt| mt < cutoff);
        if stale {
            let _ = std::fs::remove_file(entry.path());
        }
    }
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
    fn seed_jarvy_home(tmp: &tempfile::TempDir) -> PathBuf {
        // SAFETY: caller is in the `jarvy_home_env` serial group.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("JARVY_HOME", tmp.path());
        }
        let logs = tmp.path().join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        logs
    }

    fn clear_jarvy_home() {
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("JARVY_HOME");
        }
    }

    #[test]
    fn build_matcher_event_prefix_hits_structured_field() {
        let m = build_line_matcher("event=tools_d_unsafe_perms").unwrap();
        assert!(m.matches(r#"{"level":"WARN","fields":{"event":"tools_d_unsafe_perms"}}"#));
        assert!(m.matches(r#"{"fields":{"event": "tools_d_unsafe_perms","x":1}}"#));
        // Substring collision in message payload must NOT match.
        assert!(!m.matches(r#"{"fields":{"message":"tools_d_unsafe_perms happened"}}"#));
        assert_eq!(m.kind(), PatternKind::Event);
        assert_eq!(m.event_name(), Some("tools_d_unsafe_perms"));
    }

    #[test]
    fn build_matcher_bare_pattern_is_substring() {
        let m = build_line_matcher("shell_init.generated").unwrap();
        assert!(m.matches(r#"{"fields":{"event":"shell_init.generated"}}"#));
        assert!(m.matches("prefix shell_init.generated suffix"));
        assert!(!m.matches("nothing to see"));
        assert_eq!(m.kind(), PatternKind::Substring);
        assert_eq!(m.event_name(), None);
    }

    #[test]
    fn build_matcher_empty_event_name_errors() {
        // typo trap: `--filter event=` was silently a no-op
        assert!(build_line_matcher("event=").is_err());
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn strip_log_lines_removes_matches_and_skips_active() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let logs = seed_jarvy_home(&tmp);

        let rotated = logs.join("jarvy.log.2026-01-01");
        std::fs::write(
            &rotated,
            "{\"fields\":{\"event\":\"noise\"}}\n\
             {\"fields\":{\"event\":\"keep_me\"}}\n\
             {\"fields\":{\"event\":\"noise\"}}\n",
        )
        .unwrap();
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(60 * 24 * 3600);
        filetime::set_file_mtime(&rotated, filetime::FileTime::from_system_time(old)).unwrap();

        let active = logs.join("jarvy.log");
        std::fs::write(&active, "{\"fields\":{\"event\":\"noise\"}}\n").unwrap();

        let report = strip_log_lines("event=noise", 30, false, false, false).expect("strip");
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].lines_removed, 2);
        assert_eq!(report.results[0].path, rotated);
        assert_eq!(report.skipped_active, 1);

        assert_eq!(
            std::fs::read_to_string(&rotated).unwrap(),
            "{\"fields\":{\"event\":\"keep_me\"}}\n"
        );
        assert_eq!(
            std::fs::read_to_string(&active).unwrap(),
            "{\"fields\":{\"event\":\"noise\"}}\n"
        );

        clear_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn strip_log_lines_dry_run_reports_without_writing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let logs = seed_jarvy_home(&tmp);
        let rotated = logs.join("jarvy.log.2026-01-02");
        let body = "{\"fields\":{\"event\":\"drop\"}}\n{\"fields\":{\"event\":\"stay\"}}\n";
        std::fs::write(&rotated, body).unwrap();

        let report = strip_log_lines("event=drop", 30, true, true, false).expect("strip");
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].lines_removed, 1);
        assert_eq!(std::fs::read_to_string(&rotated).unwrap(), body);

        clear_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    #[cfg(unix)]
    fn strip_log_lines_refuses_symlinks() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let logs = seed_jarvy_home(&tmp);

        // Plant a symlink pointing OUTSIDE the log dir at a target the
        // strip would otherwise clobber if it followed.
        let target = tmp.path().join("SECRET.txt");
        std::fs::write(&target, "{\"fields\":{\"event\":\"noise\"}}\ndo not delete\n").unwrap();
        let link = logs.join("jarvy.log.2026-symlink");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let report = strip_log_lines("event=noise", 30, true, false, false).expect("strip");
        assert_eq!(report.results.len(), 0);
        assert_eq!(report.skipped_symlink, 1);
        // Target file untouched.
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "{\"fields\":{\"event\":\"noise\"}}\ndo not delete\n"
        );

        clear_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    #[cfg(unix)]
    fn strip_log_lines_preserves_file_mode() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().expect("tempdir");
        let logs = seed_jarvy_home(&tmp);
        let rotated = logs.join("jarvy.log.2026-mode");
        std::fs::write(
            &rotated,
            "{\"fields\":{\"event\":\"noise\"}}\n{\"fields\":{\"event\":\"keep\"}}\n",
        )
        .unwrap();
        std::fs::set_permissions(&rotated, std::fs::Permissions::from_mode(0o600)).unwrap();

        let _ = strip_log_lines("event=noise", 30, true, false, false).expect("strip");

        let mode_after = std::fs::metadata(&rotated).unwrap().permissions().mode() & 0o7777;
        assert_eq!(
            mode_after, 0o600,
            "rewrite must preserve original file mode"
        );

        clear_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn strip_log_lines_preserves_missing_trailing_newline() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let logs = seed_jarvy_home(&tmp);
        let rotated = logs.join("jarvy.log.2026-no-nl");
        // No trailing newline.
        std::fs::write(
            &rotated,
            "{\"fields\":{\"event\":\"noise\"}}\n{\"fields\":{\"event\":\"keep\"}}",
        )
        .unwrap();

        let _ = strip_log_lines("event=noise", 30, true, false, false).expect("strip");
        let after = std::fs::read_to_string(&rotated).unwrap();
        assert_eq!(after, "{\"fields\":{\"event\":\"keep\"}}");
        assert!(!after.ends_with('\n'));

        clear_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn strip_log_lines_all_lines_match_leaves_empty_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let logs = seed_jarvy_home(&tmp);
        let rotated = logs.join("jarvy.log.2026-all");
        std::fs::write(
            &rotated,
            "{\"fields\":{\"event\":\"noise\"}}\n{\"fields\":{\"event\":\"noise\"}}\n",
        )
        .unwrap();

        let report = strip_log_lines("event=noise", 30, true, false, false).expect("strip");
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].lines_removed, 2);
        assert_eq!(std::fs::read_to_string(&rotated).unwrap(), "");

        clear_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn strip_log_lines_empty_file_noop() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let logs = seed_jarvy_home(&tmp);
        let rotated = logs.join("jarvy.log.2026-empty");
        std::fs::write(&rotated, "").unwrap();

        let report = strip_log_lines("event=anything", 30, true, false, false).expect("strip");
        assert_eq!(report.results.len(), 0);
        assert_eq!(std::fs::read_to_string(&rotated).unwrap(), "");

        clear_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn strip_log_lines_without_all_skips_fresh_files() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let logs = seed_jarvy_home(&tmp);
        let fresh = logs.join("jarvy.log.2026-fresh");
        let old = logs.join("jarvy.log.2026-old");
        std::fs::write(&fresh, "{\"fields\":{\"event\":\"drop\"}}\n").unwrap();
        std::fs::write(&old, "{\"fields\":{\"event\":\"drop\"}}\n").unwrap();
        let old_mtime = std::time::SystemTime::now() - std::time::Duration::from_secs(60 * 24 * 3600);
        filetime::set_file_mtime(&old, filetime::FileTime::from_system_time(old_mtime)).unwrap();

        // all=false, max_age_days=30 → only `old` eligible.
        let report = strip_log_lines("event=drop", 30, false, false, false).expect("strip");
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].path, old);
        // Fresh file untouched.
        assert_eq!(
            std::fs::read_to_string(&fresh).unwrap(),
            "{\"fields\":{\"event\":\"drop\"}}\n"
        );

        clear_jarvy_home();
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn strip_log_lines_refuses_forensic_events() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let logs = seed_jarvy_home(&tmp);
        let rotated = logs.join("jarvy.log.2026-forensic");
        std::fs::write(
            &rotated,
            "{\"fields\":{\"event\":\"git_config.exec_key_refused\"}}\n",
        )
        .unwrap();

        // Without --allow-forensic-strip: refused, no rewrite.
        let report =
            strip_log_lines("event=git_config.exec_key_refused", 30, true, false, false)
                .expect("strip");
        assert_eq!(report.results.len(), 0);
        assert_eq!(report.forensic_refused, vec!["git_config.exec_key_refused"]);
        // File untouched.
        assert!(std::fs::read_to_string(&rotated).unwrap().contains("git_config.exec_key_refused"));

        // With allow_forensic_strip=true: proceeds.
        let report =
            strip_log_lines("event=git_config.exec_key_refused", 30, true, false, true)
                .expect("strip");
        assert_eq!(report.results.len(), 1);

        clear_jarvy_home();
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
