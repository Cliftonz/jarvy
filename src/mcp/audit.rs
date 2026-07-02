//! MCP Audit Logging
//!
//! Logs all MCP requests to an audit file for security and debugging purposes.
//!
//! ## Log Format
//!
//! Each log entry is a JSON object on a single line:
//!
//! ```json
//! {"timestamp":"2024-01-15T10:30:00Z","action":"install","tool":"docker","success":true,"version":"24.0.7","duration_ms":45000,"client":"claude-desktop"}
//! ```

use crate::mcp::config::McpConfig;
use crate::mcp::error::{McpError, McpResult};
use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

/// Audit logger for MCP operations
pub struct AuditLog {
    /// Path to the audit log file
    path: PathBuf,
    /// File handle (lazily opened)
    file: Mutex<Option<File>>,
    /// Whether logging is enabled
    enabled: bool,
}

impl AuditLog {
    /// Create a new audit logger from configuration
    pub fn new(config: &McpConfig) -> McpResult<Self> {
        let path = config.audit_log_path()?;

        Ok(Self {
            path,
            file: Mutex::new(None),
            enabled: true,
        })
    }

    /// Create a disabled audit logger (for testing)
    pub fn disabled() -> Self {
        Self {
            path: PathBuf::new(),
            file: Mutex::new(None),
            enabled: false,
        }
    }

    /// Log an audit entry
    pub fn log(&self, entry: AuditEntry) -> McpResult<()> {
        if !self.enabled {
            return Ok(());
        }

        let mut file_guard = self
            .file
            .lock()
            .map_err(|_| McpError::internal_error("Audit log lock poisoned"))?;

        // Lazy open the file
        if file_guard.is_none() {
            // Ensure parent directory exists
            if let Some(parent) = self.path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?;

            *file_guard = Some(file);
        }

        if let Some(ref mut file) = *file_guard {
            let json = serde_json::to_string(&entry)?;
            writeln!(file, "{}", json)?;
            file.flush()?;
        }

        Ok(())
    }

    /// Log a tool list request
    pub fn log_list_tools(&self, client: Option<&str>, count: usize) {
        let _ = self.log(
            AuditEntry::new(AuditAction::ListTools)
                .with_client(client)
                .with_data("count", serde_json::json!(count)),
        );
    }

    /// Log a tool check request
    pub fn log_check_tool(
        &self,
        client: Option<&str>,
        tool: &str,
        installed: bool,
        version: Option<&str>,
    ) {
        let _ = self.log(
            AuditEntry::new(AuditAction::CheckTool)
                .with_client(client)
                .with_tool(tool)
                .with_success(installed)
                .with_version(version),
        );
    }

    /// Log a tool install request (dry run)
    pub fn log_install_dry_run(&self, client: Option<&str>, tool: &str, command: &str) {
        let _ = self.log(
            AuditEntry::new(AuditAction::InstallDryRun)
                .with_client(client)
                .with_tool(tool)
                .with_data("command", serde_json::json!(command)),
        );
    }

    /// Log a tool install request (actual)
    pub fn log_install(
        &self,
        client: Option<&str>,
        tool: &str,
        success: bool,
        version: Option<&str>,
        duration_ms: u64,
        error: Option<&str>,
    ) {
        let mut entry = AuditEntry::new(AuditAction::Install)
            .with_client(client)
            .with_tool(tool)
            .with_success(success)
            .with_duration(duration_ms);

        if let Some(v) = version {
            entry = entry.with_version(Some(v));
        }
        if let Some(e) = error {
            entry = entry.with_data("error", serde_json::json!(e));
        }

        let _ = self.log(entry);
    }

    /// Log a user cancellation
    pub fn log_cancelled(&self, client: Option<&str>, tool: &str) {
        let _ = self.log(
            AuditEntry::new(AuditAction::Cancelled)
                .with_client(client)
                .with_tool(tool),
        );
    }

    /// Log a rate limit hit
    pub fn log_rate_limited(&self, client: Option<&str>, action: &str) {
        let _ = self.log(
            AuditEntry::new(AuditAction::RateLimited)
                .with_client(client)
                .with_data("attempted_action", serde_json::json!(action)),
        );
    }

    /// Log a denied tool access
    #[allow(dead_code)] // Public API for MCP audit logging
    pub fn log_denied(&self, client: Option<&str>, tool: &str, reason: &str) {
        let _ = self.log(
            AuditEntry::new(AuditAction::Denied)
                .with_client(client)
                .with_tool(tool)
                .with_data("reason", serde_json::json!(reason)),
        );
    }

    /// Log a request to invoke a mutating extended MCP tool
    /// (`jarvy_ai_hooks_apply`, `jarvy_services_start`, etc.). Records
    /// the dry_run flag so the audit trail distinguishes "preview"
    /// invocations from "actually applied" invocations.
    pub fn log_mcp_mutation(
        &self,
        client: Option<&str>,
        tool: &str,
        dry_run: bool,
        success: bool,
        details: Option<&str>,
    ) {
        let mut entry = AuditEntry::new(AuditAction::McpMutation)
            .with_client(client)
            .with_tool(tool)
            .with_success(success)
            .with_data("dry_run", serde_json::json!(dry_run));
        if let Some(d) = details {
            entry = entry.with_data("details", serde_json::json!(d));
        }
        let _ = self.log(entry);
    }

    /// Pre-flight audit record for a mutating MCP request. Emitted at
    /// the top of `gate_mutation` before rate limiting / confirmation
    /// / execution run. Distinct from `log_mcp_mutation` so an audit
    /// query "what got applied?" doesn't include requests that
    /// silently errored downstream (Sec F1 fix — previously the
    /// pre-flight wrote `mcp_mutation success=true` making failed
    /// mutations look successful).
    pub fn log_mcp_mutation_requested(
        &self,
        client: Option<&str>,
        tool: &str,
        details: Option<&str>,
    ) {
        let mut entry = AuditEntry::new(AuditAction::McpMutationRequested)
            .with_client(client)
            .with_tool(tool);
        if let Some(d) = details {
            entry = entry.with_data("details", serde_json::json!(d));
        }
        let _ = self.log(entry);
    }
}

/// Actions that can be logged
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    /// List available tools
    ListTools,
    /// Check a tool's status
    CheckTool,
    /// Dry-run install
    InstallDryRun,
    /// Actual install
    Install,
    /// User cancelled
    Cancelled,
    /// Rate limited
    RateLimited,
    /// Tool denied (allowlist/denylist)
    #[allow(dead_code)] // Used by log_denied
    Denied,
    /// Get tool info
    #[allow(dead_code)] // Reserved for MCP tool info logging
    GetTool,
    /// Read resource
    #[allow(dead_code)] // Reserved for MCP resource logging
    ReadResource,
    /// Get prompt
    #[allow(dead_code)] // Reserved for MCP prompt logging
    GetPrompt,
    /// Invocation of an extended mutating MCP tool (ai_hooks_apply,
    /// mcp_register_apply, services_start, templates_use). Records the
    /// `dry_run` flag and the per-tool result. Emitted only on a
    /// completed mutation (`Ok(())` from `gate_mutation` and
    /// downstream apply).
    McpMutation,
    /// Pre-flight audit entry for a mutating MCP tool call — the
    /// request has been received but rate-limit, confirmation, and
    /// execution have not yet run. Recorded separately from
    /// `McpMutation` so an audit query for "what actually got
    /// applied" (grep for `mcp_mutation success=true`) doesn't
    /// falsely include requests that were denied or that errored
    /// downstream. See `mcp::extended_tools::gate_mutation` for the
    /// call site.
    McpMutationRequested,
}

/// A single audit log entry
#[derive(Debug, Serialize)]
pub struct AuditEntry {
    /// ISO 8601 timestamp
    timestamp: String,
    /// The action performed
    action: AuditAction,
    /// Tool name (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    tool: Option<String>,
    /// Whether the operation succeeded
    #[serde(skip_serializing_if = "Option::is_none")]
    success: Option<bool>,
    /// Version (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    /// Duration in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_ms: Option<u64>,
    /// MCP client identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    client: Option<String>,
    /// Additional data
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

impl AuditEntry {
    /// Create a new audit entry with the current timestamp
    pub fn new(action: AuditAction) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| {
                // Simple ISO 8601 format
                let secs = d.as_secs();
                let (year, month, day, hour, min, sec) = unix_to_datetime(secs);
                format!(
                    "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                    year, month, day, hour, min, sec
                )
            })
            .unwrap_or_else(|_| "unknown".to_string());

        Self {
            timestamp,
            action,
            tool: None,
            success: None,
            version: None,
            duration_ms: None,
            client: None,
            data: None,
        }
    }

    /// Add tool name
    pub fn with_tool(mut self, tool: &str) -> Self {
        self.tool = Some(tool.to_string());
        self
    }

    /// Add success status
    pub fn with_success(mut self, success: bool) -> Self {
        self.success = Some(success);
        self
    }

    /// Add version
    pub fn with_version(mut self, version: Option<&str>) -> Self {
        self.version = version.map(|s| s.to_string());
        self
    }

    /// Add duration
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    /// Add client identifier
    pub fn with_client(mut self, client: Option<&str>) -> Self {
        self.client = client.map(|s| s.to_string());
        self
    }

    /// Add additional data
    pub fn with_data(mut self, key: &str, value: serde_json::Value) -> Self {
        let data = self.data.get_or_insert_with(|| serde_json::json!({}));
        if let Some(obj) = data.as_object_mut() {
            obj.insert(key.to_string(), value);
        }
        self
    }
}

/// Convert Unix timestamp to (year, month, day, hour, minute, second)
fn unix_to_datetime(secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    // Simple algorithm for UTC conversion
    let days = secs / 86400;
    let time = secs % 86400;

    let hour = (time / 3600) as u32;
    let min = ((time % 3600) / 60) as u32;
    let sec = (time % 60) as u32;

    // Calculate year, month, day from days since epoch (1970-01-01)
    let mut year = 1970u32;
    let mut remaining_days = days;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let leap = is_leap_year(year);
    let days_in_months: [u64; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u32;
    for days_in_month in days_in_months {
        if remaining_days < days_in_month {
            break;
        }
        remaining_days -= days_in_month;
        month += 1;
    }

    let day = (remaining_days + 1) as u32;

    (year, month, day, hour, min, sec)
}

fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_entry_serialization() {
        let entry = AuditEntry::new(AuditAction::Install)
            .with_tool("git")
            .with_success(true)
            .with_version(Some("2.43.0"))
            .with_duration(1234)
            .with_client(Some("claude-desktop"));

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"action\":\"install\""));
        assert!(json.contains("\"tool\":\"git\""));
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"version\":\"2.43.0\""));
        assert!(json.contains("\"duration_ms\":1234"));
        assert!(json.contains("\"client\":\"claude-desktop\""));
    }

    #[test]
    fn test_audit_entry_with_data() {
        let entry =
            AuditEntry::new(AuditAction::ListTools).with_data("count", serde_json::json!(85));

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"count\":85"));
    }

    #[test]
    fn test_unix_to_datetime() {
        // 2024-01-15T12:36:40Z
        let (year, month, day, hour, min, sec) = unix_to_datetime(1705322200);
        assert_eq!(year, 2024);
        assert_eq!(month, 1);
        assert_eq!(day, 15);
        assert_eq!(hour, 12);
        assert_eq!(min, 36);
        assert_eq!(sec, 40);
    }

    #[test]
    fn test_disabled_audit_log() {
        let log = AuditLog::disabled();
        // Should not error even when disabled
        log.log_list_tools(Some("test"), 10);
        log.log_check_tool(Some("test"), "git", true, Some("2.43.0"));
    }
}
