//! MCP Tool Handlers
//!
//! Implements the MCP tool interface for Jarvy:
//! - jarvy_list_tools: List all tools Jarvy can install
//! - jarvy_get_tool: Get detailed information about a specific tool
//! - jarvy_check_tool: Check if a tool is installed and get its version
//! - jarvy_check_multiple: Check installation status of multiple tools
//! - jarvy_install_tool: Install a development tool (with confirmation)

use crate::mcp::audit::AuditLog;
use crate::mcp::config::McpConfig;
use crate::mcp::error::{McpError, McpResult};
use crate::mcp::safety::{self, ConfirmationResult, RateLimiter};
use crate::tools::spec::{ToolIndexEntry, generate_tool_index, get_tool_spec};
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Tool definition for MCP tools/list response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// Input schema
    pub input_schema: serde_json::Value,
}

/// List all MCP tools exposed by Jarvy
pub fn list_tools() -> Vec<McpToolDefinition> {
    let mut tools: Vec<McpToolDefinition> = vec![
        McpToolDefinition {
            name: "jarvy_get_install_instructions".to_string(),
            description: "Get instructions for installing Jarvy itself on any platform. Use this when Jarvy is not yet installed.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "platform": {
                        "type": "string",
                        "enum": ["macos", "linux", "windows", "current"],
                        "description": "Target platform (default: current)"
                    },
                    "method": {
                        "type": "string",
                        "enum": ["curl", "brew", "cargo", "winget", "chocolatey", "all"],
                        "description": "Preferred installation method (default: all available)"
                    }
                }
            }),
        },
        McpToolDefinition {
            name: "jarvy_check_self".to_string(),
            description: "Check Jarvy's own version and installation status".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        McpToolDefinition {
            name: "jarvy_list_tools".to_string(),
            description: "List all tools Jarvy can install, with optional filtering".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "category": {
                        "type": "string",
                        "enum": ["language", "database", "container", "cli", "editor", "all"],
                        "description": "Filter by tool category"
                    },
                    "platform": {
                        "type": "string",
                        "enum": ["macos", "linux", "windows", "current"],
                        "description": "Filter by platform support (default: current)"
                    },
                    "search": {
                        "type": "string",
                        "description": "Search tools by name"
                    }
                }
            }),
        },
        McpToolDefinition {
            name: "jarvy_get_tool".to_string(),
            description: "Get detailed information about a specific tool".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Tool name (e.g., 'git', 'docker', 'node')"
                    }
                },
                "required": ["name"]
            }),
        },
        McpToolDefinition {
            name: "jarvy_check_tool".to_string(),
            description: "Check if a tool is installed and get its version".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Tool name to check"
                    }
                },
                "required": ["name"]
            }),
        },
        McpToolDefinition {
            name: "jarvy_check_multiple".to_string(),
            description: "Check installation status of multiple tools at once".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "tools": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of tool names to check"
                    }
                },
                "required": ["tools"]
            }),
        },
        McpToolDefinition {
            name: "jarvy_install_tool".to_string(),
            description: "Install a development tool (requires user confirmation)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Tool name to install"
                    },
                    "version": {
                        "type": "string",
                        "description": "Version hint (default: 'latest')"
                    },
                    "dry_run": {
                        "type": "boolean",
                        "description": "Preview installation without executing (default: true)"
                    }
                },
                "required": ["name"]
            }),
        },
    ];
    tools.extend(crate::mcp::extended_tools::extended_definitions());
    tools
}

/// Handle jarvy_get_install_instructions request
pub fn handle_get_install_instructions(
    arguments: Option<serde_json::Value>,
) -> McpResult<serde_json::Value> {
    #[derive(Deserialize, Default)]
    struct Params {
        #[serde(default)]
        platform: Option<String>,
        #[serde(default)]
        method: Option<String>,
    }

    let params: Params = arguments
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default();

    let platform = params.platform.as_deref().unwrap_or("current");
    let target_platform = if platform == "current" {
        get_current_platform()
    } else {
        platform.to_string()
    };

    let method_filter = params.method.as_deref().unwrap_or("all");

    let mut methods: Vec<serde_json::Value> = Vec::new();

    // Curl/shell script (Unix)
    if matches!(target_platform.as_str(), "macos" | "linux")
        && matches!(method_filter, "all" | "curl")
    {
        methods.push(serde_json::json!({
            "method": "curl",
            "command": "curl -fsSL https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.sh | bash",
            "description": "Quick install via shell script (recommended)",
            "requires_sudo": false,
            "notes": "Downloads latest release and installs to ~/.local/bin"
        }));
    }

    // PowerShell script (Windows)
    if target_platform == "windows" && matches!(method_filter, "all" | "curl") {
        methods.push(serde_json::json!({
            "method": "powershell",
            "command": "irm https://raw.githubusercontent.com/Cliftonz/jarvy/main/dist/scripts/install.ps1 | iex",
            "description": "Quick install via PowerShell script (recommended)",
            "requires_sudo": false,
            "notes": "Downloads latest release and adds to PATH"
        }));
    }

    // Homebrew (macOS/Linux)
    if matches!(target_platform.as_str(), "macos" | "linux")
        && matches!(method_filter, "all" | "brew")
    {
        methods.push(serde_json::json!({
            "method": "homebrew",
            "command": "brew install Cliftonz/tap/jarvy",
            "description": "Install via Homebrew",
            "requires_sudo": false,
            "notes": "Requires Homebrew to be installed first"
        }));
    }

    // Cargo (all platforms)
    if matches!(method_filter, "all" | "cargo") {
        methods.push(serde_json::json!({
            "method": "cargo",
            "command": "cargo install jarvy",
            "description": "Install from crates.io via Cargo",
            "requires_sudo": false,
            "notes": "Requires Rust toolchain to be installed"
        }));
    }

    // Winget (Windows)
    if target_platform == "windows" && matches!(method_filter, "all" | "winget") {
        methods.push(serde_json::json!({
            "method": "winget",
            "command": "winget install Jarvy.Jarvy",
            "description": "Install via Windows Package Manager",
            "requires_sudo": false,
            "notes": "Available on Windows 10/11 with winget"
        }));
    }

    // Chocolatey (Windows)
    if target_platform == "windows" && matches!(method_filter, "all" | "chocolatey") {
        methods.push(serde_json::json!({
            "method": "chocolatey",
            "command": "choco install jarvy",
            "description": "Install via Chocolatey",
            "requires_sudo": true,
            "notes": "Run from elevated PowerShell"
        }));
    }

    // Determine recommended method
    let recommended = match target_platform.as_str() {
        "macos" | "linux" => "curl",
        "windows" => "powershell",
        _ => "cargo",
    };

    Ok(serde_json::json!({
        "platform": target_platform,
        "methods": methods,
        "recommended": recommended,
        "project_url": "https://github.com/Cliftonz/jarvy",
        "documentation": "https://github.com/Cliftonz/jarvy#readme"
    }))
}

/// Handle jarvy_check_self request
pub fn handle_check_self() -> McpResult<serde_json::Value> {
    let version = env!("CARGO_PKG_VERSION");
    let platform = get_current_platform();
    let arch = std::env::consts::ARCH;

    // Check if jarvy binary is in PATH
    let binary_path = std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().to_string());

    // Get config directory
    let config_dir = crate::paths::jarvy_home()
        .ok()
        .map(|p| p.to_string_lossy().to_string());

    let config_exists = config_dir
        .as_ref()
        .map(|p| std::path::Path::new(p).exists())
        .unwrap_or(false);

    // Check for jarvy.toml in current directory
    let local_config_exists = std::path::Path::new("jarvy.toml").exists();

    Ok(serde_json::json!({
        "installed": true,
        "version": version,
        "platform": platform,
        "arch": arch,
        "binary_path": binary_path,
        "config_dir": config_dir,
        "config_initialized": config_exists,
        "local_config_exists": local_config_exists,
        "update_command": "jarvy upgrade --self"
    }))
}

/// Handle jarvy_list_tools request
pub fn handle_list_tools(arguments: Option<serde_json::Value>) -> McpResult<serde_json::Value> {
    #[derive(Deserialize, Default)]
    struct Params {
        #[serde(default)]
        category: Option<String>,
        #[serde(default)]
        platform: Option<String>,
        #[serde(default)]
        search: Option<String>,
    }

    let params: Params = arguments
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default();

    let index = generate_tool_index();
    let current_platform = get_current_platform();

    let tools: Vec<ToolSummary> = index
        .tools
        .into_iter()
        .filter(|t| {
            // Filter by search term
            if let Some(ref search) = params.search
                && !t.name.to_lowercase().contains(&search.to_lowercase())
            {
                return false;
            }

            // Filter by platform
            let platform_filter = params.platform.as_deref().unwrap_or("current");
            if !matches_platform(t, platform_filter, &current_platform) {
                return false;
            }

            // Filter by category
            if let Some(ref category) = params.category {
                match t.category.as_deref() {
                    Some(cat) if cat.eq_ignore_ascii_case(category) => {}
                    _ => return false,
                }
            }

            true
        })
        .map(|t| ToolSummary {
            name: t.name.clone(),
            command: t.command.clone(),
            platforms: get_supported_platforms(&t),
            has_custom_installer: t.custom_install.has_custom_installer,
        })
        .collect();

    let count = tools.len();

    Ok(serde_json::json!({
        "tools": tools,
        "count": count,
        "platform": current_platform
    }))
}

/// Handle jarvy_get_tool request
pub fn handle_get_tool(arguments: Option<serde_json::Value>) -> McpResult<serde_json::Value> {
    #[derive(Deserialize)]
    struct Params {
        name: String,
    }

    let params: Params = arguments
        .map(serde_json::from_value)
        .transpose()?
        .ok_or_else(|| McpError::invalid_params("Missing 'name' parameter"))?;

    let spec = get_tool_spec(&params.name).ok_or_else(|| McpError::unknown_tool(&params.name))?;

    let current_platform = get_current_platform();
    let index = generate_tool_index();
    let tool_entry = index.tools.iter().find(|t| t.name == params.name);

    let current_platform_info = tool_entry.and_then(|t| match current_platform.as_str() {
        "macos" => t.macos.as_ref().map(|m| {
            serde_json::json!({
                "os": "macos",
                "install_method": if m.cask.is_some() { "cask" } else { "brew" },
                "package_name": m.cask.or(m.brew).unwrap_or("unknown"),
                "package_manager": "homebrew"
            })
        }),
        "linux" => t.linux.as_ref().map(|l| {
            serde_json::json!({
                "os": "linux",
                "install_method": "package_manager",
                "package_name": l.apt
                    .or(l.dnf)
                    .or(l.pacman)
                    .or(l.apk)
                    .or(l.brew)
                    .unwrap_or("unknown"),
                "package_manager": "system"
            })
        }),
        "windows" => t.windows.as_ref().map(|w| {
            serde_json::json!({
                "os": "windows",
                "install_method": "winget",
                "package_name": w.winget.unwrap_or("unknown"),
                "package_manager": "winget"
            })
        }),
        _ => None,
    });

    Ok(serde_json::json!({
        "name": params.name,
        "command": spec.command,
        "current_platform": current_platform_info,
        "all_platforms": tool_entry.map(|t| serde_json::json!({
            "macos": t.macos,
            "linux": t.linux,
            "windows": t.windows
        })),
        "custom_install": tool_entry.map(|t| t.custom_install.has_custom_installer).unwrap_or(false),
    }))
}

/// Handle jarvy_check_tool request
pub fn handle_check_tool(arguments: Option<serde_json::Value>) -> McpResult<serde_json::Value> {
    #[derive(Deserialize)]
    struct Params {
        name: String,
    }

    let params: Params = arguments
        .map(serde_json::from_value)
        .transpose()?
        .ok_or_else(|| McpError::invalid_params("Missing 'name' parameter"))?;

    let spec = get_tool_spec(&params.name).ok_or_else(|| McpError::unknown_tool(&params.name))?;

    // Check if tool is installed
    let version = get_installed_version(spec.command);
    let path = which_path(spec.command);

    Ok(serde_json::json!({
        "name": params.name,
        "installed": version.is_some(),
        "version": version,
        "path": path
    }))
}

/// Handle jarvy_check_multiple request
pub fn handle_check_multiple(arguments: Option<serde_json::Value>) -> McpResult<serde_json::Value> {
    #[derive(Deserialize)]
    struct Params {
        tools: Vec<String>,
    }

    let params: Params = arguments
        .map(serde_json::from_value)
        .transpose()?
        .ok_or_else(|| McpError::invalid_params("Missing 'tools' parameter"))?;

    let results: Vec<serde_json::Value> = params
        .tools
        .iter()
        .map(|name| {
            if let Some(spec) = get_tool_spec(name) {
                let version = get_installed_version(spec.command);
                let path = which_path(spec.command);
                serde_json::json!({
                    "name": name,
                    "installed": version.is_some(),
                    "version": version,
                    "path": path
                })
            } else {
                serde_json::json!({
                    "name": name,
                    "installed": false,
                    "error": "Unknown tool"
                })
            }
        })
        .collect();

    let installed_count = results
        .iter()
        .filter(|r| r["installed"].as_bool().unwrap_or(false))
        .count();

    Ok(serde_json::json!({
        "results": results,
        "total": params.tools.len(),
        "installed": installed_count,
        "missing": params.tools.len() - installed_count
    }))
}

/// Handle jarvy_install_tool request
pub fn handle_install_tool(
    arguments: Option<serde_json::Value>,
    config: &McpConfig,
    rate_limiter: &RateLimiter,
    audit_log: &AuditLog,
    client_name: Option<&str>,
) -> McpResult<serde_json::Value> {
    #[derive(Deserialize)]
    struct Params {
        name: String,
        #[serde(default)]
        version: Option<String>,
        #[serde(default)]
        dry_run: Option<bool>,
    }

    let params: Params = arguments
        .map(serde_json::from_value)
        .transpose()?
        .ok_or_else(|| McpError::invalid_params("Missing 'name' parameter"))?;

    // Safety checks
    safety::check_allowlist(&params.name, config)?;

    let spec = get_tool_spec(&params.name).ok_or_else(|| McpError::unknown_tool(&params.name))?;

    // Get install command info
    let install_info = get_install_info(&params.name);

    // Dry run is the default (safe by default)
    let dry_run = params.dry_run.unwrap_or(true);

    if dry_run {
        audit_log.log_install_dry_run(client_name, &params.name, &install_info.command);
        return Ok(serde_json::json!({
            "name": params.name,
            "dry_run": true,
            "would_execute": {
                "command": install_info.command,
                "package_manager": install_info.package_manager,
                "requires_sudo": install_info.requires_sudo
            },
            "notes": "Set dry_run to false and confirm to proceed with installation"
        }));
    }

    // Rate limit check for actual installs
    rate_limiter.check_install_limit().inspect_err(|_e| {
        audit_log.log_rate_limited(client_name, "install_tool");
    })?;

    // Check if confirmation should be skipped
    // Check global auto-approve preference from ~/.jarvy/config.toml
    let global_auto_approve = crate::init::initialize().mcp.auto_approve_installs;
    let skip_confirm = config.skip_confirmation(&params.name)
        || !config.mcp.require_confirmation
        || global_auto_approve;

    if !skip_confirm {
        // Prompt for confirmation
        match safety::prompt_user_confirmation(&params.name, &install_info.command, client_name)? {
            ConfirmationResult::Yes => {
                // Continue with install
            }
            ConfirmationResult::No => {
                audit_log.log_cancelled(client_name, &params.name);
                return Err(McpError::user_cancelled());
            }
            ConfirmationResult::Always => {
                // Persist "always allow" preference to ~/.jarvy/config.toml.
                // Audit event records the security state change so a debug
                // ticket can show when blanket auto-approval was enabled.
                match crate::init::modify_global_config(|cfg| {
                    cfg.mcp.auto_approve_installs = true;
                }) {
                    Ok(()) => tracing::info!(
                        event = "mcp.auto_approve.enabled",
                        tool = %params.name,
                        client = client_name.unwrap_or("unknown"),
                    ),
                    Err(e) => tracing::warn!(
                        event = "mcp.auto_approve.persist_failed",
                        tool = %params.name,
                        error = %e,
                    ),
                }
                // Continue with install
            }
        }
    }

    // Execute installation
    let start = Instant::now();
    let version = params.version.as_deref().unwrap_or("latest");

    match crate::tools::add(&params.name, version) {
        Ok(()) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            let installed_version = get_installed_version(spec.command);

            audit_log.log_install(
                client_name,
                &params.name,
                true,
                installed_version.as_deref(),
                duration_ms,
                None,
            );

            Ok(serde_json::json!({
                "name": params.name,
                "success": true,
                "installed_version": installed_version,
                "duration_ms": duration_ms
            }))
        }
        Err(e) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            let error_msg = format!("{:?}", e);

            audit_log.log_install(
                client_name,
                &params.name,
                false,
                None,
                duration_ms,
                Some(&error_msg),
            );

            Err(McpError::installation_failed(&params.name, error_msg))
        }
    }
}

/// Tool summary for list response
#[derive(Debug, Serialize)]
struct ToolSummary {
    name: String,
    command: String,
    platforms: Vec<String>,
    has_custom_installer: bool,
}

/// Installation info for dry-run response
struct InstallInfo {
    command: String,
    package_manager: String,
    requires_sudo: bool,
}

/// Get the current platform
fn get_current_platform() -> String {
    #[cfg(target_os = "macos")]
    return "macos".to_string();
    #[cfg(target_os = "linux")]
    return "linux".to_string();
    #[cfg(target_os = "windows")]
    return "windows".to_string();
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    return "unknown".to_string();
}

/// Check if a tool matches the platform filter
fn matches_platform(tool: &ToolIndexEntry, filter: &str, current: &str) -> bool {
    match filter {
        "current" => match current {
            "macos" => tool.macos.is_some() || tool.custom_install.has_custom_installer,
            "linux" => tool.linux.is_some() || tool.custom_install.has_custom_installer,
            "windows" => tool.windows.is_some() || tool.custom_install.has_custom_installer,
            _ => true,
        },
        "macos" => tool.macos.is_some() || tool.custom_install.has_custom_installer,
        "linux" => tool.linux.is_some() || tool.custom_install.has_custom_installer,
        "windows" => tool.windows.is_some() || tool.custom_install.has_custom_installer,
        "all" => true,
        _ => true,
    }
}

/// Get supported platforms for a tool
fn get_supported_platforms(tool: &ToolIndexEntry) -> Vec<String> {
    let mut platforms = Vec::new();
    if tool.macos.is_some() || tool.custom_install.has_custom_installer {
        platforms.push("macos".to_string());
    }
    if tool.linux.is_some() || tool.custom_install.has_custom_installer {
        platforms.push("linux".to_string());
    }
    if tool.windows.is_some() || tool.custom_install.has_custom_installer {
        platforms.push("windows".to_string());
    }
    platforms
}

/// Get installed version of a command
fn get_installed_version(command: &str) -> Option<String> {
    // Try common version flags
    for flag in &["--version", "-v", "-V", "version"] {
        if let Ok(output) = std::process::Command::new(command).arg(flag).output()
            && output.status.success()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}{}", stdout, stderr);

            // Extract version number (simple regex-free approach)
            if let Some(version) = extract_version(&combined) {
                return Some(version);
            }
        }
    }
    None
}

/// Extract version number from output
fn extract_version(output: &str) -> Option<String> {
    // Look for patterns like x.y.z, vx.y.z
    for word in output.split_whitespace() {
        let word = word.trim_start_matches('v').trim_end_matches(',');
        if word.chars().next().is_some_and(|c| c.is_ascii_digit())
            && word.contains('.')
            && word.chars().all(|c| c.is_ascii_digit() || c == '.')
        {
            return Some(word.to_string());
        }
    }
    None
}

/// Get the path to a command
fn which_path(command: &str) -> Option<String> {
    std::process::Command::new("which")
        .arg(command)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
}

/// Get installation info for a tool
fn get_install_info(tool_name: &str) -> InstallInfo {
    let index = generate_tool_index();
    let tool = index.tools.iter().find(|t| t.name == tool_name);

    let current_platform = get_current_platform();

    if let Some(tool) = tool {
        match current_platform.as_str() {
            "macos" => {
                if let Some(ref macos) = tool.macos {
                    if let Some(ref cask) = macos.cask {
                        return InstallInfo {
                            command: format!("brew install --cask {}", cask),
                            package_manager: "homebrew".to_string(),
                            requires_sudo: false,
                        };
                    }
                    if let Some(ref brew) = macos.brew {
                        return InstallInfo {
                            command: format!("brew install {}", brew),
                            package_manager: "homebrew".to_string(),
                            requires_sudo: false,
                        };
                    }
                }
            }
            "linux" => {
                if let Some(ref linux) = tool.linux {
                    if let Some(ref apt) = linux.apt {
                        return InstallInfo {
                            command: format!("sudo apt install -y {}", apt),
                            package_manager: "apt".to_string(),
                            requires_sudo: true,
                        };
                    }
                    if let Some(ref dnf) = linux.dnf {
                        return InstallInfo {
                            command: format!("sudo dnf install -y {}", dnf),
                            package_manager: "dnf".to_string(),
                            requires_sudo: true,
                        };
                    }
                    if let Some(ref pacman) = linux.pacman {
                        return InstallInfo {
                            command: format!("sudo pacman -S --noconfirm {}", pacman),
                            package_manager: "pacman".to_string(),
                            requires_sudo: true,
                        };
                    }
                }
            }
            "windows" => {
                if let Some(ref windows) = tool.windows
                    && let Some(ref winget) = windows.winget
                {
                    return InstallInfo {
                        command: format!("winget install {}", winget),
                        package_manager: "winget".to_string(),
                        requires_sudo: false,
                    };
                }
            }
            _ => {}
        }
    }

    // Fallback
    InstallInfo {
        command: format!("jarvy setup (tool: {})", tool_name),
        package_manager: "jarvy".to_string(),
        requires_sudo: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_tools_returns_definitions() {
        let tools = list_tools();
        assert!(!tools.is_empty());
        assert!(tools.iter().any(|t| t.name == "jarvy_list_tools"));
        assert!(tools.iter().any(|t| t.name == "jarvy_check_tool"));
        assert!(tools.iter().any(|t| t.name == "jarvy_install_tool"));
    }

    #[test]
    fn test_handle_list_tools() {
        // Initialize the tool registry
        crate::tools::register_all();

        let result = handle_list_tools(None).unwrap();
        assert!(result.get("tools").is_some());
        assert!(result.get("count").is_some());
        assert!(result.get("platform").is_some());
    }

    #[test]
    fn test_handle_check_tool_unknown() {
        let result = handle_check_tool(Some(serde_json::json!({"name": "nonexistent_tool_xyz"})));
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_version() {
        assert_eq!(
            extract_version("git version 2.43.0"),
            Some("2.43.0".to_string())
        );
        assert_eq!(extract_version("v1.2.3"), Some("1.2.3".to_string()));
        assert_eq!(
            extract_version("Docker version 24.0.7, build afdd53b"),
            Some("24.0.7".to_string())
        );
        assert_eq!(extract_version("no version here"), None);
    }

    #[test]
    fn test_get_current_platform() {
        let platform = get_current_platform();
        #[cfg(target_os = "macos")]
        assert_eq!(platform, "macos");
        #[cfg(target_os = "linux")]
        assert_eq!(platform, "linux");
        #[cfg(target_os = "windows")]
        assert_eq!(platform, "windows");
    }
}
