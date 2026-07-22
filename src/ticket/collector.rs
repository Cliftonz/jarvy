//! Data collectors for ticket generation
//!
//! Collects system, tool, configuration, and log information.

use std::collections::HashMap;
use std::path::PathBuf;

use super::{TicketData, TicketError, TicketScope};
use crate::logging;

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

        // Allowlist of safe environment variables to include
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
