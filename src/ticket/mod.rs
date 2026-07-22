//! Debug ticket generation for support and diagnostics
//!
//! This module provides:
//! - Collection of system, tool, and configuration information
//! - Bundling into sanitized ZIP archives
//! - CLI commands for ticket management

mod bundler;
mod collector;

pub use bundler::{TicketBundler, preview_ticket};
pub use collector::{SystemInfo, TicketCollector, ToolInfo};

use std::path::PathBuf;
use thiserror::Error;

/// Ticket generation errors
#[derive(Debug, Error)]
pub enum TicketError {
    #[error("Failed to collect system info: {0}")]
    CollectionFailed(String),

    #[error("Failed to create ticket directory: {0}")]
    DirectoryCreationFailed(#[from] std::io::Error),

    #[error("Failed to create ZIP archive: {0}")]
    ArchiveCreationFailed(String),

    #[error("Failed to read configuration: {0}")]
    ConfigReadFailed(String),

    #[error("Invalid ticket ID: {0}")]
    InvalidTicketId(String),
}

/// Scope of data to include in a ticket
#[derive(Debug, Clone, Default)]
pub struct TicketScope {
    /// Include system information
    pub system: bool,
    /// Include tool status
    pub tools: bool,
    /// Include configuration (sanitized)
    pub config: bool,
    /// Include environment variables (filtered)
    pub environment: bool,
    /// Include recent logs
    pub logs: bool,
    /// Number of log lines to include
    pub log_lines: usize,
    /// Specific tool to focus on (optional)
    pub tool_filter: Option<String>,
}

impl TicketScope {
    /// Create a scope for a full diagnostic ticket
    pub fn full() -> Self {
        Self {
            system: true,
            tools: true,
            config: true,
            environment: true,
            logs: true,
            log_lines: 500,
            tool_filter: None,
        }
    }

    /// Create a scope focused on a specific tool
    pub fn for_tool(tool: &str) -> Self {
        Self {
            system: true,
            tools: true,
            config: true,
            environment: true,
            logs: true,
            log_lines: 200,
            tool_filter: Some(tool.to_string()),
        }
    }

    /// Create a minimal scope (system info only)
    pub fn minimal() -> Self {
        Self {
            system: true,
            tools: false,
            config: false,
            environment: false,
            logs: false,
            log_lines: 0,
            tool_filter: None,
        }
    }
}

/// Complete ticket data ready for bundling
#[derive(Debug, Clone, serde::Serialize)]
pub struct TicketData {
    /// Unique ticket identifier
    pub ticket_id: String,
    /// When the ticket was created
    pub created_at: String,
    /// Jarvy version
    pub jarvy_version: String,
    /// System information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemInfo>,
    /// Tool status information
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolInfo>,
    /// Configuration (sanitized)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
    /// Environment variables (filtered)
    #[serde(skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub environment: std::collections::HashMap<String, String>,
    /// Recent log entries
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub logs: Vec<String>,
}

impl TicketData {
    /// Create a new ticket with a generated ID
    pub fn new() -> Self {
        let ticket_id = format!(
            "JARVY-{}-{}",
            chrono::Utc::now().format("%Y%m%d"),
            &uuid::Uuid::now_v7().to_string()[..8]
        );

        Self {
            ticket_id,
            created_at: chrono::Utc::now().to_rfc3339(),
            jarvy_version: env!("CARGO_PKG_VERSION").to_string(),
            system: None,
            tools: Vec::new(),
            config: None,
            environment: std::collections::HashMap::new(),
            logs: Vec::new(),
        }
    }
}

impl Default for TicketData {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the default tickets directory (~/.jarvy/tickets/) via the canonical
/// resolver so a `JARVY_HOME` override is honored.
pub fn default_tickets_directory() -> PathBuf {
    crate::paths::tickets_dir().unwrap_or_else(|_| PathBuf::from(".jarvy/tickets"))
}

/// List existing tickets
pub fn list_tickets() -> Result<Vec<(String, PathBuf, u64)>, TicketError> {
    let tickets_dir = default_tickets_directory();
    let mut tickets = Vec::new();

    if !tickets_dir.exists() {
        return Ok(tickets);
    }

    for entry in (std::fs::read_dir(&tickets_dir)?).flatten() {
        let path = entry.path();
        if path.is_file()
            && path.extension().map(|e| e == "zip").unwrap_or(false)
            && let Some(name) = path.file_stem()
        {
            let size = path.metadata().map(|m| m.len()).unwrap_or(0);
            tickets.push((name.to_string_lossy().to_string(), path, size));
        }
    }

    // Sort by name (which includes date)
    tickets.sort_by(|a, b| b.0.cmp(&a.0));

    Ok(tickets)
}

/// Clean old tickets based on age
pub fn clean_tickets(max_age_days: u32) -> Result<(usize, u64), TicketError> {
    let tickets_dir = default_tickets_directory();
    let max_age_secs = max_age_days as u64 * 24 * 60 * 60;
    let now = std::time::SystemTime::now();

    let mut removed_count = 0;
    let mut removed_bytes = 0;

    if !tickets_dir.exists() {
        return Ok((0, 0));
    }

    for entry in (std::fs::read_dir(&tickets_dir)?).flatten() {
        let path = entry.path();
        if path.is_file()
            && path.extension().map(|e| e == "zip").unwrap_or(false)
            && let Ok(metadata) = path.metadata()
            && let Ok(modified) = metadata.modified()
            && let Ok(age) = now.duration_since(modified)
            && age.as_secs() > max_age_secs
        {
            removed_bytes += metadata.len();
            if std::fs::remove_file(&path).is_ok() {
                removed_count += 1;
            }
        }
    }

    Ok((removed_count, removed_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ticket_data_new() {
        let ticket = TicketData::new();
        assert!(ticket.ticket_id.starts_with("JARVY-"));
        assert!(!ticket.created_at.is_empty());
    }

    #[test]
    fn test_ticket_scope_full() {
        let scope = TicketScope::full();
        assert!(scope.system);
        assert!(scope.tools);
        assert!(scope.config);
        assert!(scope.logs);
        assert_eq!(scope.log_lines, 500);
    }

    #[test]
    fn test_ticket_scope_for_tool() {
        let scope = TicketScope::for_tool("git");
        assert_eq!(scope.tool_filter, Some("git".to_string()));
    }

    #[test]
    fn test_ticket_scope_minimal() {
        let scope = TicketScope::minimal();
        assert!(scope.system);
        assert!(!scope.tools);
        assert!(!scope.logs);
    }

    /// Serialized against `jarvy_home_env` because
    /// `default_tickets_directory()` reads JARVY_HOME — concurrent
    /// tests that pin tempdirs into JARVY_HOME otherwise race with
    /// the `.jarvy/tickets` suffix assertion.
    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn test_default_tickets_directory() {
        let dir = default_tickets_directory();
        assert!(dir.ends_with(".jarvy/tickets"));
    }
}
