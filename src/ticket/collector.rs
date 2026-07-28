//! Data collectors for ticket generation
//!
//! Collects system, tool, configuration, and log information.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use super::{TicketData, TicketError, TicketScope};
use crate::logging;

/// Directory names under `~/.jarvy` that a debug bundle must NEVER ship.
/// `agent-profiles/` holds per-profile agent homes (the targets of
/// `CLAUDE_CONFIG_DIR` / `CODEX_HOME`) with LIVE credentials (PRD-058).
/// Collection is narrow today (two config files + the log file), so this
/// is mostly a guard for the future: any collector that starts walking
/// `~/.jarvy` must consult `is_excluded_path` before reading.
pub(crate) const TICKET_EXCLUDED_DIRS: &[&str] = &["agent-profiles"];

/// True when any path component names an excluded directory.
///
/// Component match, not substring — `agent-profiles.toml` is fine,
/// `agent-profiles/work/...` is not.
pub(crate) fn is_excluded_path(path: &Path) -> bool {
    path.components().any(|c| match c {
        Component::Normal(name) => TICKET_EXCLUDED_DIRS
            .iter()
            .any(|dir| name == std::ffi::OsStr::new(dir)),
        _ => false,
    })
}

/// System information
#[derive(Debug, Clone, serde::Serialize)]
pub struct SystemInfo {
    pub os_name: String,
    pub os_version: String,
    pub os_release: String,
    pub architecture: String,
    pub cpu_cores: usize,
    pub memory_total_mb: u64,
    pub shell: String,
    pub locale: String,
    pub home_directory: String,
    pub hostname: String,
}

/// Tool status information
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub error: Option<String>,
}

/// Ticket data collector
pub struct TicketCollector {
    scope: TicketScope,
    sanitizer: logging::Sanitizer,
}

impl TicketCollector {
    /// Create a new collector with the given scope
    pub fn new(scope: TicketScope) -> Self {
        Self {
            scope,
            sanitizer: logging::Sanitizer::new(),
        }
    }

    /// Collect all data according to scope
    pub fn collect(&self) -> Result<TicketData, TicketError> {
        let mut ticket = TicketData::new();

        if self.scope.system {
            ticket.system = Some(self.collect_system_info()?);
        }

        if self.scope.tools {
            ticket.tools = self.collect_tool_info()?;
        }

        if self.scope.config {
            ticket.config = self.collect_config()?;
        }

        if self.scope.environment {
            ticket.environment = self.collect_environment();
        }

        if self.scope.logs && self.scope.log_lines > 0 {
            ticket.logs = self.collect_logs(self.scope.log_lines)?;
        }

        Ok(ticket)
    }

    /// Collect system information
    fn collect_system_info(&self) -> Result<SystemInfo, TicketError> {
        let os_name = std::env::consts::OS.to_string();
        let architecture = std::env::consts::ARCH.to_string();

        // Get OS version/release using sys-info
        let (os_version, os_release) = match sys_info::os_release() {
            Ok(release) => (
                sys_info::os_type().unwrap_or_else(|_| "unknown".to_string()),
                release,
            ),
            Err(_) => ("unknown".to_string(), "unknown".to_string()),
        };

        // Get CPU and memory info
        let cpu_cores = num_cpus::get();
        let memory_total_mb = sys_info::mem_info().map(|m| m.total / 1024).unwrap_or(0);

        // Get shell
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "unknown".to_string());

        // Get locale
        let locale = std::env::var("LANG")
            .or_else(|_| std::env::var("LC_ALL"))
            .unwrap_or_else(|_| "unknown".to_string());

        // Get home directory (sanitized to ~)
        let home_directory = "~".to_string();

        // Get hostname
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        Ok(SystemInfo {
            os_name,
            os_version,
            os_release,
            architecture,
            cpu_cores,
            memory_total_mb,
            shell,
            locale,
            home_directory,
            hostname,
        })
    }

    /// Collect tool status information
    fn collect_tool_info(&self) -> Result<Vec<ToolInfo>, TicketError> {
        let mut tools = Vec::new();

        // Get list of tools to check from the registry
        let tool_names = crate::tools::registered_tool_names();

        for name in tool_names {
            // Filter by tool if specified
            if let Some(ref filter) = self.scope.tool_filter
                && !name.eq_ignore_ascii_case(filter)
            {
                continue;
            }

            let mut tool_info = ToolInfo {
                name: name.clone(),
                installed: false,
                version: None,
                path: None,
                error: None,
            };

            // Check if tool is installed using which
            if let Ok(path) = which::which(&name) {
                tool_info.installed = true;
                tool_info.path = Some(self.sanitize_path(&path));

                // Try to get version
                if let Ok(output) = std::process::Command::new(&name).arg("--version").output()
                    && output.status.success()
                {
                    let version_output = String::from_utf8_lossy(&output.stdout);
                    // Take first line and sanitize
                    if let Some(first_line) = version_output.lines().next() {
                        // sanitize() already returns String — drop
                        // the redundant to_string() that doubled
                        // the alloc (round-2 perf F10).
                        tool_info.version = Some(self.sanitizer.sanitize(first_line));
                    }
                }
            }

            tools.push(tool_info);
        }

        Ok(tools)
    }

    /// Collect configuration (sanitized)
    fn collect_config(&self) -> Result<Option<serde_json::Value>, TicketError> {
        // Try to read jarvy.toml (project) and ~/.jarvy/config.toml (global).
        let config_paths = [
            PathBuf::from("jarvy.toml"),
            crate::paths::config_toml().unwrap_or_else(|_| PathBuf::new()),
        ];

        for path in &config_paths {
            // Defensive: the fixed paths above never point into an
            // excluded dir today, but the guard keeps future additions
            // (or a hostile JARVY_HOME layout) honest — profiles hold
            // live credentials (PRD-058).
            if is_excluded_path(path) {
                continue;
            }
            if path.exists() {
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        // Sanitize the content
                        let sanitized = self.sanitizer.sanitize(&content);

                        // Parse as TOML and convert to JSON for consistent output
                        match toml::from_str::<toml::Value>(&sanitized) {
                            Ok(toml_value) => {
                                // Convert TOML to JSON
                                let json_value = toml_to_json(toml_value);
                                return Ok(Some(json_value));
                            }
                            Err(_) => {
                                // Return as raw string if TOML parsing fails
                                return Ok(Some(serde_json::json!({
                                    "raw": sanitized.to_string(),
                                    "parse_error": true
                                })));
                            }
                        }
                    }
                    Err(_) => continue,
                }
            }
        }

        Ok(None)
    }

    /// Collect filtered environment variables
    fn collect_environment(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();

        // Allowlist of safe environment variables to include.
        // Do NOT add CLAUDE_CONFIG_DIR / CODEX_HOME (or other agent-home
        // overrides): their values reveal agent-profile names and paths
        // under ~/.jarvy/agent-profiles/ (PRD-058).
        let allowlist = [
            "SHELL",
            "TERM",
            "LANG",
            "LC_ALL",
            "PATH",
            "EDITOR",
            "VISUAL",
            "HOME",
            "USER",
            "LOGNAME",
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_CACHE_HOME",
            "HOMEBREW_PREFIX",
            "CARGO_HOME",
            "RUSTUP_HOME",
            "GOPATH",
            "GOROOT",
            "NVM_DIR",
            "PYENV_ROOT",
            "JAVA_HOME",
            "NODE_PATH",
            "CI",
            "GITHUB_ACTIONS",
            "GITLAB_CI",
            "JENKINS_URL",
            "JARVY_TEST_MODE",
        ];

        for key in allowlist {
            if let Ok(value) = std::env::var(key) {
                // Sanitize the value
                let sanitized = self.sanitizer.sanitize(&value);
                env.insert(key.to_string(), sanitized.to_string());
            }
        }

        env
    }

    /// Collect recent log entries
    fn collect_logs(&self, lines: usize) -> Result<Vec<String>, TicketError> {
        match logging::read_recent_logs(lines) {
            Ok(logs) => {
                // Sanitize each log line
                // sanitize() already returns String — drop the redundant
                // to_string() that doubled the per-line alloc (round-2
                // perf F10). 1k log bundle: ~1MB less heap churn.
                Ok(logs
                    .into_iter()
                    .map(|l| self.sanitizer.sanitize(&l))
                    .collect())
            }
            Err(e) => {
                // Don't fail the whole ticket for log errors
                tracing::warn!("Failed to collect logs: {}", e);
                Ok(Vec::new())
            }
        }
    }

    /// Sanitize a path (replace home directory with ~)
    fn sanitize_path(&self, path: &std::path::Path) -> String {
        self.sanitizer.sanitize(&path.to_string_lossy())
    }
}

/// Convert TOML value to JSON value
fn toml_to_json(toml: toml::Value) -> serde_json::Value {
    match toml {
        toml::Value::String(s) => serde_json::Value::String(s),
        toml::Value::Integer(i) => serde_json::Value::Number(i.into()),
        toml::Value::Float(f) => serde_json::Number::from_f64(f)
            .map_or(serde_json::Value::Null, serde_json::Value::Number),
        toml::Value::Boolean(b) => serde_json::Value::Bool(b),
        toml::Value::Datetime(dt) => serde_json::Value::String(dt.to_string()),
        toml::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(toml_to_json).collect())
        }
        toml::Value::Table(table) => {
            let map: serde_json::Map<String, serde_json::Value> = table
                .into_iter()
                .map(|(k, v)| (k, toml_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_system_info() {
        let collector = TicketCollector::new(TicketScope::minimal());
        let info = collector.collect_system_info().unwrap();

        assert!(!info.os_name.is_empty());
        assert!(!info.architecture.is_empty());
        assert!(info.cpu_cores > 0);
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_collect_environment() {
        // SAFETY: Test environment modification is safe in single-threaded tests
        unsafe { std::env::set_var("SHELL", "/bin/bash") };
        let collector = TicketCollector::new(TicketScope::full());
        let env = collector.collect_environment();

        assert!(env.contains_key("SHELL"));
    }

    #[test]
    fn test_tool_info_defaults() {
        let info = ToolInfo {
            name: "test".to_string(),
            installed: false,
            version: None,
            path: None,
            error: None,
        };

        assert!(!info.installed);
        assert!(info.version.is_none());
    }

    #[test]
    fn test_is_excluded_path() {
        // Anywhere in the path, any depth.
        assert!(is_excluded_path(Path::new(
            "/Users/x/.jarvy/agent-profiles/work/claude-code/credentials.json"
        )));
        assert!(is_excluded_path(Path::new(".jarvy/agent-profiles")));
        assert!(is_excluded_path(Path::new("agent-profiles")));

        // Component match only — similar names and siblings pass.
        assert!(!is_excluded_path(Path::new("/Users/x/.jarvy/config.toml")));
        assert!(!is_excluded_path(Path::new(
            "/Users/x/.jarvy/agent-profiles.toml"
        )));
        assert!(!is_excluded_path(Path::new(
            "/Users/x/.jarvy/logs/jarvy.log"
        )));
        assert!(!is_excluded_path(Path::new("jarvy.toml")));
    }

    #[test]
    fn test_is_excluded_path_nested_and_relative_depths() {
        // Deeply nested files under the excluded dir are caught at any
        // depth, absolute or relative.
        assert!(is_excluded_path(Path::new(
            "agent-profiles/work/claude-code/nested/deeper/.credentials.json"
        )));
        assert!(is_excluded_path(Path::new(
            "/home/u/.jarvy/agent-profiles/p/codex/auth.json"
        )));
        // Excluded component mid-path, not just as the leading segment.
        assert!(is_excluded_path(Path::new(
            "backup/.jarvy/agent-profiles/x"
        )));
    }

    #[test]
    fn test_is_excluded_path_no_prefix_or_suffix_confusion() {
        // Sibling dirs sharing the prefix must NOT be excluded —
        // component equality, not starts_with.
        assert!(!is_excluded_path(Path::new(
            "/Users/x/.jarvy/agent-profiles-x/creds.json"
        )));
        assert!(!is_excluded_path(Path::new("agent-profiles-backup/file")));
        // ...and suffix confusion.
        assert!(!is_excluded_path(Path::new(
            "/Users/x/.jarvy/old-agent-profiles/file"
        )));
        assert!(!is_excluded_path(Path::new("agent-profilesx")));
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_env_allowlist_omits_agent_profile_vars() {
        // CLAUDE_CONFIG_DIR / CODEX_HOME values reveal profile names and
        // paths under ~/.jarvy/agent-profiles/ — they must never enter
        // the allowlist (PRD-058).
        // SAFETY: no other test reads these variables.
        unsafe {
            std::env::set_var(
                "CLAUDE_CONFIG_DIR",
                "/tmp/.jarvy/agent-profiles/work/claude-code",
            );
            std::env::set_var("CODEX_HOME", "/tmp/.jarvy/agent-profiles/work/codex");
        }
        let collector = TicketCollector::new(TicketScope::full());
        let env = collector.collect_environment();
        unsafe {
            std::env::remove_var("CLAUDE_CONFIG_DIR");
            std::env::remove_var("CODEX_HOME");
        }
        assert!(!env.contains_key("CLAUDE_CONFIG_DIR"));
        assert!(!env.contains_key("CODEX_HOME"));
    }

    /// Regression (PRD-058): a populated `$JARVY_HOME/agent-profiles/`
    /// tree must leave zero trace in the collected TicketData.
    /// Serialized on `jarvy_home_env` — see ticket/mod.rs.
    #[test]
    #[allow(unsafe_code)]
    #[serial_test::serial(jarvy_home_env)]
    fn test_collect_never_includes_agent_profiles() {
        let home = tempfile::tempdir().unwrap();
        let creds_dir = home.path().join("agent-profiles/work/claude-code");
        std::fs::create_dir_all(&creds_dir).unwrap();
        std::fs::write(
            creds_dir.join("credentials.json"),
            r#"{"api_key":"JARVY-TEST-SUPER-SECRET-TOKEN"}"#,
        )
        .unwrap();
        // A legitimate global config so config collection has real work.
        std::fs::write(
            home.path().join("config.toml"),
            "[telemetry]\nenabled = false\n",
        )
        .unwrap();

        // SAFETY: serialized on jarvy_home_env; restored before asserts.
        let prev = std::env::var_os("JARVY_HOME");
        unsafe { std::env::set_var("JARVY_HOME", home.path()) };

        let scope = TicketScope {
            config: true,
            environment: true,
            logs: true,
            log_lines: 100,
            ..Default::default()
        };
        let result = TicketCollector::new(scope).collect();

        match prev {
            Some(v) => unsafe { std::env::set_var("JARVY_HOME", v) },
            None => unsafe { std::env::remove_var("JARVY_HOME") },
        }

        let ticket = result.unwrap();
        let json = serde_json::to_string(&ticket).unwrap();
        assert!(!json.contains("JARVY-TEST-SUPER-SECRET-TOKEN"));
        assert!(!json.contains("credentials.json"));
        assert!(!json.contains("agent-profiles"));
    }

    #[test]
    fn test_toml_to_json() {
        let toml_value = toml::Value::Table({
            let mut table = toml::map::Map::new();
            table.insert("key".to_string(), toml::Value::String("value".to_string()));
            table
        });

        let json = toml_to_json(toml_value);
        assert_eq!(json["key"], "value");
    }
}
