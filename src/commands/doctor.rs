//! Environment health diagnostics
//!
//! Diagnose environment issues, check tool health, and verify PATH configuration.
//!
//! ## Extended Dashboard (PRD-027 T11)
//!
//! The `--extended` flag adds comprehensive system metrics:
//! - System overview (OS, shell, uptime, load, memory, disk)
//! - Package manager status with package counts
//! - Performance metrics and trends
//! - Detailed tool version comparison

use crate::config::Config;
use crate::output::{ExitCode, Outputable, colors, header, icons, subheader};
use crate::telemetry;
use crate::tools::common::{cmd_satisfies, has};
use crate::tools::spec::{
    DependencyCheckResult, check_tool_dependencies, get_tool_default_hook, get_tool_dependencies,
    get_tool_flexible_dependencies, get_tool_spec, should_ignore_missing_deps,
};
use serde::Serialize;
use std::collections::HashSet;
use std::env;
use std::io::Write;
use std::path::Path;

/// System information
#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    pub os: String,
    pub os_version: String,
    pub arch: String,
    pub shell: String,
    pub home: String,
    pub package_manager: Option<String>,
}

/// PATH check result
#[derive(Debug, Clone, Serialize)]
pub struct PathCheck {
    pub path: String,
    pub status: PathStatus,
    pub in_path: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PathStatus {
    Ok,
    Missing,
    NotInPath,
}

/// Tool health status
#[derive(Debug, Clone, Serialize)]
pub struct ToolHealth {
    pub name: String,
    pub required: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed: Option<String>,
    pub status: ToolStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Dependency satisfaction status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<DependencyInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolStatus {
    Ok,
    Outdated,
    NotInstalled,
    Unknown,
}

/// Dependency satisfaction status for a tool
#[derive(Debug, Clone, Serialize)]
pub struct DependencyInfo {
    /// Whether all dependencies are satisfied
    pub satisfied: bool,
    /// For flexible deps: which installed tool satisfies the requirement
    #[serde(skip_serializing_if = "Option::is_none")]
    pub satisfied_by: Option<String>,
    /// Missing strict dependencies
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_required: Vec<String>,
    /// Missing flexible dependency options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing_flexible: Option<FlexibleDepInfo>,
    /// A flexible dependency will be installed from config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub will_install: Option<String>,
}

/// Info about missing flexible dependencies
#[derive(Debug, Clone, Serialize)]
pub struct FlexibleDepInfo {
    /// Available options to satisfy the dependency
    pub options: Vec<String>,
    /// Suggested option to install
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

/// Hook status check
#[derive(Debug, Clone, Serialize)]
pub struct HookStatus {
    pub name: String,
    pub description: String,
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
}

/// Recommendation for fixing issues
#[derive(Debug, Clone, Serialize)]
pub struct Recommendation {
    pub severity: RecommendationSeverity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RecommendationSeverity {
    Error,
    Warning,
    Info,
}

/// Complete doctor result
#[derive(Debug, Clone, Serialize)]
pub struct DoctorResult {
    pub system: SystemInfo,
    pub path_checks: Vec<PathCheck>,
    pub tools: Vec<ToolHealth>,
    pub hooks: Vec<HookStatus>,
    pub recommendations: Vec<Recommendation>,
    pub exit_code: i32,
}

impl Outputable for DoctorResult {
    fn to_human(&self) -> String {
        let mut output = String::new();

        output.push_str(&header("Jarvy Doctor"));
        output.push('\n');

        // System Information
        output.push_str(&subheader("System Information"));
        output.push_str(&format!(
            "  OS: {} {} ({})\n",
            self.system.os, self.system.os_version, self.system.arch
        ));
        output.push_str(&format!("  Shell: {}\n", self.system.shell));
        if let Some(ref pm) = self.system.package_manager {
            output.push_str(&format!("  Package Manager: {}\n", pm));
        }

        // PATH Analysis
        if !self.path_checks.is_empty() {
            output.push_str(&subheader("PATH Analysis"));
            for check in &self.path_checks {
                let (icon, color) = match check.status {
                    PathStatus::Ok => (icons::OK, colors::GREEN),
                    PathStatus::Missing => (icons::ERROR, colors::RED),
                    PathStatus::NotInPath => (icons::WARN, colors::YELLOW),
                };
                let status_msg = if check.in_path {
                    "in PATH"
                } else {
                    "not in PATH"
                };
                output.push_str(&format!(
                    "  {}{}{} {} - {}\n",
                    color,
                    icon,
                    colors::RESET,
                    check.path,
                    status_msg
                ));
            }
        }

        // Tool Health
        if !self.tools.is_empty() {
            output.push_str(&subheader("Tool Health"));
            for tool in &self.tools {
                let (icon, color) = match tool.status {
                    ToolStatus::Ok => (icons::OK, colors::GREEN),
                    ToolStatus::Outdated => (icons::WARN, colors::YELLOW),
                    ToolStatus::NotInstalled => (icons::ERROR, colors::RED),
                    ToolStatus::Unknown => (icons::INFO, colors::CYAN),
                };

                let installed_str = tool
                    .installed
                    .as_ref()
                    .map(|v| format!(" (installed: {})", v))
                    .unwrap_or_else(|| " - not found".to_string());

                let status_msg = match tool.status {
                    ToolStatus::Ok => "satisfies requirement",
                    ToolStatus::Outdated => "outdated",
                    ToolStatus::NotInstalled => "not installed",
                    ToolStatus::Unknown => "unknown tool",
                };

                output.push_str(&format!(
                    "  {}{}{} {} {}{} - {}\n",
                    color,
                    icon,
                    colors::RESET,
                    tool.name,
                    tool.required,
                    installed_str,
                    status_msg
                ));

                // Show dependency status if present
                if let Some(ref deps) = tool.dependencies {
                    if deps.satisfied {
                        if let Some(ref satisfied_by) = deps.satisfied_by {
                            output.push_str(&format!(
                                "      {}↳ dependencies satisfied by: {}{}\n",
                                colors::DIM,
                                satisfied_by,
                                colors::RESET
                            ));
                        }
                    } else if !deps.missing_required.is_empty() {
                        output.push_str(&format!(
                            "      {}{}↳ MISSING required: {}{}\n",
                            colors::RED,
                            icons::ERROR,
                            deps.missing_required.join(", "),
                            colors::RESET
                        ));
                    } else if let Some(ref will_install) = deps.will_install {
                        output.push_str(&format!(
                            "      {}↳ will install dependency: {}{}\n",
                            colors::CYAN,
                            will_install,
                            colors::RESET
                        ));
                    } else if let Some(ref flex) = deps.missing_flexible {
                        output.push_str(&format!(
                            "      {}{}↳ needs one of: {}{}\n",
                            colors::YELLOW,
                            icons::WARN,
                            flex.options.join(", "),
                            colors::RESET
                        ));
                        if let Some(ref suggestion) = flex.suggestion {
                            output.push_str(&format!(
                                "      {}  suggested: jarvy setup --only {}{}\n",
                                colors::DIM,
                                suggestion,
                                colors::RESET
                            ));
                        }
                    }
                }
            }
        }

        // Hooks Status
        if !self.hooks.is_empty() {
            output.push_str(&subheader("Hooks Status"));
            for hook in &self.hooks {
                let (icon, color) = if hook.active {
                    (icons::OK, colors::GREEN)
                } else {
                    (icons::WARN, colors::YELLOW)
                };
                output.push_str(&format!(
                    "  {}{}{} {}: {}\n",
                    color,
                    icon,
                    colors::RESET,
                    hook.name,
                    hook.description
                ));
                if let Some(ref issue) = hook.issue {
                    output.push_str(&format!(
                        "      {}{}{}\n",
                        colors::DIM,
                        issue,
                        colors::RESET
                    ));
                }
            }
        }

        // Recommendations
        if !self.recommendations.is_empty() {
            output.push_str(&subheader("Recommendations"));
            for (i, rec) in self.recommendations.iter().enumerate() {
                let color = match rec.severity {
                    RecommendationSeverity::Error => colors::RED,
                    RecommendationSeverity::Warning => colors::YELLOW,
                    RecommendationSeverity::Info => colors::CYAN,
                };
                output.push_str(&format!(
                    "  {}{}. {}{}\n",
                    color,
                    i + 1,
                    rec.message,
                    colors::RESET
                ));
                if let Some(ref fix) = rec.fix {
                    output.push_str(&format!("     Fix: {}\n", fix));
                }
            }
        }

        output
    }

    fn exit_code(&self) -> ExitCode {
        match self.exit_code {
            0 => ExitCode::Ok,
            1 => ExitCode::Warning,
            _ => ExitCode::Error,
        }
    }
}

/// Run the doctor command
/// A `jarvy doctor --check <category>` filter. System info is always
/// shown as context, so it is not a filterable category; the checkable
/// sections are PATH analysis, tool health, and hook status.
///
/// (The PRD-027 sketch also listed "network" / "performance" categories,
/// but the `network_trace` module was deleted as unwired and profiling
/// lives on `jarvy setup --profile` — neither maps to a doctor section,
/// so they are intentionally omitted.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorCategory {
    Path,
    Tools,
    Hooks,
}

impl DoctorCategory {
    /// Parse a comma-separated `--check` value into a deduplicated list.
    /// Returns `Err` naming the offending token (and the valid set) on
    /// the first unrecognized category.
    pub fn parse_list(raw: &str) -> Result<Vec<DoctorCategory>, String> {
        let mut out: Vec<DoctorCategory> = Vec::new();
        for token in raw.split(',') {
            let t = token.trim();
            if t.is_empty() {
                continue;
            }
            let cat = match t.to_ascii_lowercase().as_str() {
                "path" | "paths" => DoctorCategory::Path,
                "tool" | "tools" => DoctorCategory::Tools,
                "hook" | "hooks" => DoctorCategory::Hooks,
                other => {
                    return Err(format!(
                        "unknown doctor category `{other}` (valid: path, tools, hooks)"
                    ));
                }
            };
            if !out.contains(&cat) {
                out.push(cat);
            }
        }
        if out.is_empty() {
            return Err("no valid categories in --check (valid: path, tools, hooks)".to_string());
        }
        Ok(out)
    }
}

/// `true` when `cat` should run: no filter means every category runs.
fn wants(filter: Option<&[DoctorCategory]>, cat: DoctorCategory) -> bool {
    filter.is_none_or(|f| f.contains(&cat))
}

pub fn run_doctor(config: Option<&Config>, specific_tools: Option<Vec<String>>) -> DoctorResult {
    run_doctor_filtered(config, specific_tools, None)
}

/// Category-filtered doctor run. Unselected sections are skipped entirely
/// (no probing) and come back empty, which the human/JSON renderers
/// already hide. System info is always collected as context.
pub fn run_doctor_filtered(
    config: Option<&Config>,
    specific_tools: Option<Vec<String>>,
    categories: Option<&[DoctorCategory]>,
) -> DoctorResult {
    let system = collect_system_info();
    let path_checks = if wants(categories, DoctorCategory::Path) {
        check_path_entries()
    } else {
        Vec::new()
    };

    // Get tools to check
    let tools_to_check: Vec<(String, String)> = if let Some(tools) = specific_tools {
        // Specific tools requested
        tools
            .iter()
            .map(|t| (t.clone(), "latest".to_string()))
            .collect()
    } else if let Some(cfg) = config {
        // From config file
        cfg.get_tool_configs()
            .values()
            .map(|t| (t.name.clone(), t.version.clone()))
            .collect()
    } else {
        // Default: check common tools
        vec![
            ("git".to_string(), "latest".to_string()),
            ("node".to_string(), "latest".to_string()),
            ("python".to_string(), "latest".to_string()),
        ]
    };

    // Build set of tools in config for dependency checking
    let config_tools: HashSet<String> = tools_to_check
        .iter()
        .map(|(name, _)| name.to_lowercase())
        .collect();

    let tools = if wants(categories, DoctorCategory::Tools) {
        check_tool_health(&tools_to_check, &config_tools)
    } else {
        Vec::new()
    };
    let hooks = if wants(categories, DoctorCategory::Hooks) {
        check_hook_status(config)
    } else {
        Vec::new()
    };
    let recommendations = generate_recommendations(&path_checks, &tools, &hooks);

    // Calculate exit code - also consider dependency issues
    let has_errors = tools.iter().any(|t| t.status == ToolStatus::NotInstalled)
        || tools.iter().any(|t| {
            t.dependencies
                .as_ref()
                .is_some_and(|d| !d.missing_required.is_empty())
        })
        || recommendations
            .iter()
            .any(|r| r.severity == RecommendationSeverity::Error);
    let has_warnings = tools.iter().any(|t| t.status == ToolStatus::Outdated)
        || tools.iter().any(|t| {
            t.dependencies
                .as_ref()
                .is_some_and(|d| d.missing_flexible.is_some())
        })
        || recommendations
            .iter()
            .any(|r| r.severity == RecommendationSeverity::Warning);

    let exit_code = if has_errors {
        2
    } else if has_warnings {
        1
    } else {
        0
    };

    // Emit telemetry
    telemetry::doctor_completed(recommendations.len(), tools.len(), exit_code);

    DoctorResult {
        system,
        path_checks,
        tools,
        hooks,
        recommendations,
        exit_code,
    }
}

fn collect_system_info() -> SystemInfo {
    let os = if cfg!(target_os = "macos") {
        "macOS".to_string()
    } else if cfg!(target_os = "linux") {
        "Linux".to_string()
    } else if cfg!(target_os = "windows") {
        "Windows".to_string()
    } else {
        "Unknown".to_string()
    };

    let os_version = get_os_version();

    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64".to_string()
    } else if cfg!(target_arch = "aarch64") {
        "arm64".to_string()
    } else {
        std::env::consts::ARCH.to_string()
    };

    let shell = env::var("SHELL").unwrap_or_else(|_| "unknown".to_string());
    let home = env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .unwrap_or_else(|_| "unknown".to_string());

    let package_manager = detect_package_manager();

    SystemInfo {
        os,
        os_version,
        arch,
        shell,
        home,
        package_manager,
    }
}

fn get_os_version() -> String {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|l| l.starts_with("VERSION_ID="))
                    .map(|l| {
                        l.trim_start_matches("VERSION_ID=")
                            .trim_matches('"')
                            .to_string()
                    })
            })
            .unwrap_or_else(|| "unknown".to_string())
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "ver"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        "unknown".to_string()
    }
}

fn detect_package_manager() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        if has("brew") {
            // Get brew version
            let version = std::process::Command::new("brew")
                .arg("--version")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .and_then(|s| s.lines().next().map(|l| l.to_string()))
                .unwrap_or_else(|| "Homebrew".to_string());
            return Some(version);
        }
    }
    #[cfg(target_os = "linux")]
    {
        if has("apt") {
            return Some("apt (Debian/Ubuntu)".to_string());
        }
        if has("dnf") {
            return Some("dnf (Fedora/RHEL)".to_string());
        }
        if has("pacman") {
            return Some("pacman (Arch)".to_string());
        }
        if has("apk") {
            return Some("apk (Alpine)".to_string());
        }
    }
    #[cfg(target_os = "windows")]
    {
        if has("winget") {
            return Some("winget".to_string());
        }
        if has("choco") {
            return Some("Chocolatey".to_string());
        }
    }
    None
}

fn check_path_entries() -> Vec<PathCheck> {
    let mut checks = Vec::new();

    // Common paths to check
    let paths_to_check = get_expected_paths();

    let current_path = env::var("PATH").unwrap_or_default();
    let path_entries: Vec<&str> = current_path.split(':').collect();

    for expected_path in paths_to_check {
        let exists = Path::new(&expected_path).exists();
        let in_path = path_entries.iter().any(|p| *p == expected_path);

        let status = if !exists {
            PathStatus::Missing
        } else if !in_path {
            PathStatus::NotInPath
        } else {
            PathStatus::Ok
        };

        checks.push(PathCheck {
            path: expected_path,
            status,
            in_path,
        });
    }

    checks
}

fn get_expected_paths() -> Vec<String> {
    let home = env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
    let mut paths = Vec::new();

    #[cfg(target_os = "macos")]
    {
        paths.push("/opt/homebrew/bin".to_string());
        paths.push("/usr/local/bin".to_string());
    }

    paths.push(format!("{}/.cargo/bin", home));
    paths.push(format!("{}/.local/bin", home));
    paths.push(format!("{}/.nvm/current/bin", home));

    #[cfg(target_os = "linux")]
    {
        paths.push("/usr/bin".to_string());
        paths.push("/usr/local/bin".to_string());
    }

    paths
}

fn check_tool_health(
    tools: &[(String, String)],
    config_tools: &HashSet<String>,
) -> Vec<ToolHealth> {
    // First pass: collect which tools are installed
    let installed_tools: HashSet<String> = tools
        .iter()
        .filter_map(|(name, _)| {
            let spec = get_tool_spec(name);
            let command = spec.map(|s| s.command).unwrap_or(name.as_str());
            if has(command) {
                Some(name.to_lowercase())
            } else {
                None
            }
        })
        .collect();

    // Second pass: check health and dependencies
    tools
        .iter()
        .map(|(name, version)| {
            let spec = get_tool_spec(name);
            let is_known = spec.is_some() || crate::tools::get_tool(name).is_some();

            if !is_known {
                return ToolHealth {
                    name: name.clone(),
                    required: version.clone(),
                    installed: None,
                    status: ToolStatus::Unknown,
                    path: None,
                    dependencies: None,
                };
            }

            let command = spec.map(|s| s.command).unwrap_or(name.as_str());
            let installed = get_installed_version(command);
            let path = which_command(command);

            let status = if installed.is_none() {
                ToolStatus::NotInstalled
            } else if cmd_satisfies(command, version) {
                ToolStatus::Ok
            } else {
                ToolStatus::Outdated
            };

            // Check dependencies
            let dependencies = check_tool_dependency_status(name, config_tools, &installed_tools);

            ToolHealth {
                name: name.clone(),
                required: version.clone(),
                installed,
                status,
                path,
                dependencies,
            }
        })
        .collect()
}

/// Check dependencies for a tool and return DependencyInfo
fn check_tool_dependency_status(
    tool_name: &str,
    config_tools: &HashSet<String>,
    installed_tools: &HashSet<String>,
) -> Option<DependencyInfo> {
    let strict_deps = get_tool_dependencies(tool_name);
    let flex_deps = get_tool_flexible_dependencies(tool_name);

    // If tool has no dependencies, return None
    if strict_deps.is_empty() && flex_deps.is_empty() {
        return None;
    }

    let result = check_tool_dependencies(tool_name, config_tools, installed_tools);

    match result {
        DependencyCheckResult::Satisfied => {
            // Check if it was satisfied by a flexible dependency
            let satisfied_by = if !flex_deps.is_empty() {
                flex_deps
                    .iter()
                    .find(|dep| installed_tools.contains(&dep.to_lowercase()))
                    .map(|s| s.to_string())
            } else {
                None
            };

            Some(DependencyInfo {
                satisfied: true,
                satisfied_by,
                missing_required: vec![],
                missing_flexible: None,
                will_install: None,
            })
        }
        DependencyCheckResult::MissingRequired(missing) => Some(DependencyInfo {
            satisfied: false,
            satisfied_by: None,
            missing_required: missing,
            missing_flexible: None,
            will_install: None,
        }),
        DependencyCheckResult::WillInstallFlexible(tool) => Some(DependencyInfo {
            satisfied: false,
            satisfied_by: None,
            missing_required: vec![],
            missing_flexible: None,
            will_install: Some(tool),
        }),
        DependencyCheckResult::MissingFlexible {
            needed: _,
            options,
            suggestion,
        } => Some(DependencyInfo {
            satisfied: false,
            satisfied_by: None,
            missing_required: vec![],
            missing_flexible: Some(FlexibleDepInfo {
                options,
                suggestion,
            }),
            will_install: None,
        }),
    }
}

fn get_installed_version(command: &str) -> Option<String> {
    // Try common version flags
    for flag in ["--version", "-v", "-V", "version"] {
        if let Ok(output) = std::process::Command::new(command).arg(flag).output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let combined = format!("{}{}", stdout, stderr);

                // Extract version number
                if let Some(version) = extract_version(&combined) {
                    return Some(version);
                }
            }
        }
    }
    None
}

fn extract_version(text: &str) -> Option<String> {
    let re = regex::Regex::new(r"v?(\d+\.\d+(?:\.\d+)?)").ok()?;
    re.captures(text).map(|c| c[1].to_string())
}

fn which_command(command: &str) -> Option<String> {
    #[cfg(unix)]
    {
        std::process::Command::new("which")
            .arg(command)
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    String::from_utf8(o.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                } else {
                    None
                }
            })
    }
    #[cfg(windows)]
    {
        std::process::Command::new("where")
            .arg(command)
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    String::from_utf8(o.stdout)
                        .ok()
                        .and_then(|s| s.lines().next().map(|l| l.trim().to_string()))
                } else {
                    None
                }
            })
    }
}

fn check_hook_status(_config: Option<&Config>) -> Vec<HookStatus> {
    let mut statuses = Vec::new();
    let home = env::var("HOME").unwrap_or_default();

    // Check common shell integrations
    let shell_rc = env::var("SHELL").ok().and_then(|s| {
        if s.contains("zsh") {
            Some(format!("{}/.zshrc", home))
        } else if s.contains("bash") {
            Some(format!("{}/.bashrc", home))
        } else {
            None
        }
    });

    if let Some(rc_path) = &shell_rc {
        let rc_content = std::fs::read_to_string(rc_path).unwrap_or_default();

        // Check for common integrations
        let integrations = [
            ("starship", "starship init", "Starship prompt"),
            ("zoxide", "zoxide init", "Zoxide directory jumper"),
            ("nvm", "nvm.sh", "Node Version Manager"),
            ("direnv", "direnv hook", "Directory environment"),
            ("fzf", "fzf", "Fuzzy finder"),
        ];

        for (name, pattern, desc) in integrations {
            if has(name) {
                let active = rc_content.contains(pattern);
                statuses.push(HookStatus {
                    name: name.to_string(),
                    description: desc.to_string(),
                    active,
                    issue: if !active {
                        Some(format!("{} not initialized in {}", name, rc_path))
                    } else {
                        None
                    },
                });
            }
        }
    }

    statuses
}

fn generate_recommendations(
    path_checks: &[PathCheck],
    tools: &[ToolHealth],
    hooks: &[HookStatus],
) -> Vec<Recommendation> {
    let mut recommendations = Vec::new();

    // Recommendations for missing tools
    for tool in tools {
        if tool.status == ToolStatus::NotInstalled {
            recommendations.push(Recommendation {
                severity: RecommendationSeverity::Error,
                message: format!("Install {}", tool.name),
                fix: Some(format!("jarvy setup --only {}", tool.name)),
            });
        } else if tool.status == ToolStatus::Outdated {
            recommendations.push(Recommendation {
                severity: RecommendationSeverity::Warning,
                message: format!("Update {} to {}", tool.name, tool.required),
                fix: Some(format!("jarvy upgrade {}", tool.name)),
            });
        }

        // Recommendations for dependency issues (unless ignored)
        if let Some(ref deps) = tool.dependencies {
            if !should_ignore_missing_deps() {
                if !deps.missing_required.is_empty() {
                    recommendations.push(Recommendation {
                        severity: RecommendationSeverity::Error,
                        message: format!(
                            "{} requires: {}",
                            tool.name,
                            deps.missing_required.join(", ")
                        ),
                        fix: Some(format!(
                            "jarvy setup --only {}",
                            deps.missing_required.join(" ")
                        )),
                    });
                }

                if let Some(ref flex) = deps.missing_flexible {
                    let suggestion = flex
                        .suggestion
                        .as_deref()
                        .unwrap_or(flex.options.first().map(|s| s.as_str()).unwrap_or(""));
                    recommendations.push(Recommendation {
                        severity: RecommendationSeverity::Warning,
                        message: format!("{} needs one of: {}", tool.name, flex.options.join(", ")),
                        fix: Some(format!("jarvy setup --only {}", suggestion)),
                    });
                }
            }
        }
    }

    // Recommendations for PATH issues
    for check in path_checks {
        if check.status == PathStatus::NotInPath {
            recommendations.push(Recommendation {
                severity: RecommendationSeverity::Warning,
                message: format!("{} not in PATH", check.path),
                fix: Some(format!(
                    "Add 'export PATH=\"{}:$PATH\"' to your shell rc",
                    check.path
                )),
            });
        }
    }

    // Recommendations for inactive hooks
    for hook in hooks {
        if !hook.active {
            if let Some(ref issue) = hook.issue {
                let default_hook = get_tool_default_hook(&hook.name);
                let fix = default_hook
                    .map(|h| format!("Run the default hook or add manually: {}", h.description));

                recommendations.push(Recommendation {
                    severity: RecommendationSeverity::Info,
                    message: issue.clone(),
                    fix,
                });
            }
        }
    }

    recommendations
}

// =============================================================================
// Extended Dashboard (PRD-027 T11)
// =============================================================================

/// Extended system metrics for --extended flag
#[derive(Debug, Clone, Serialize, Default)]
pub struct ExtendedMetrics {
    /// System uptime in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime_secs: Option<u64>,
    /// Load averages (1, 5, 15 minutes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_avg: Option<(f64, f64, f64)>,
    /// Total memory in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_total: Option<u64>,
    /// Used memory in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_used: Option<u64>,
    /// Disk total in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_total: Option<u64>,
    /// Disk available in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_available: Option<u64>,
    /// Package manager package count
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_count: Option<usize>,
    /// Outdated packages count
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outdated_count: Option<usize>,
}

/// Complete doctor result with extended metrics
#[derive(Debug, Clone, Serialize)]
pub struct DoctorResultExtended {
    #[serde(flatten)]
    pub base: DoctorResult,
    /// Extended system metrics (only with --extended)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extended: Option<ExtendedMetrics>,
    /// Tool status summary
    pub summary: ToolSummary,
}

/// Tool status summary counts
#[derive(Debug, Clone, Serialize)]
pub struct ToolSummary {
    pub total: usize,
    pub healthy: usize,
    pub outdated: usize,
    pub missing: usize,
    pub unknown: usize,
}

impl Outputable for DoctorResultExtended {
    fn to_human(&self) -> String {
        let mut output = String::new();

        output.push_str(&header("Jarvy Doctor (Extended)"));
        output.push('\n');

        // System Information
        output.push_str(&subheader("System Information"));
        output.push_str(&format!(
            "  OS: {} {} ({})\n",
            self.base.system.os, self.base.system.os_version, self.base.system.arch
        ));
        output.push_str(&format!("  Shell: {}\n", self.base.system.shell));
        if let Some(ref pm) = self.base.system.package_manager {
            output.push_str(&format!("  Package Manager: {}\n", pm));
        }

        // Extended metrics if available
        if let Some(ref ext) = self.extended {
            output.push_str(&subheader("System Metrics"));

            if let Some(uptime) = ext.uptime_secs {
                let days = uptime / 86400;
                let hours = (uptime % 86400) / 3600;
                let mins = (uptime % 3600) / 60;
                output.push_str(&format!("  Uptime: {}d {}h {}m\n", days, hours, mins));
            }

            if let Some((l1, l5, l15)) = ext.load_avg {
                output.push_str(&format!(
                    "  Load Average: {:.2}, {:.2}, {:.2}\n",
                    l1, l5, l15
                ));
            }

            if let (Some(total), Some(used)) = (ext.memory_total, ext.memory_used) {
                let pct = (used as f64 / total as f64) * 100.0;
                output.push_str(&format!(
                    "  Memory: {:.1} GB / {:.1} GB ({:.0}%)\n",
                    used as f64 / 1_000_000_000.0,
                    total as f64 / 1_000_000_000.0,
                    pct
                ));
            }

            if let (Some(total), Some(avail)) = (ext.disk_total, ext.disk_available) {
                let used = total - avail;
                let pct = (used as f64 / total as f64) * 100.0;
                output.push_str(&format!(
                    "  Disk: {:.1} GB / {:.1} GB ({:.0}%)\n",
                    used as f64 / 1_000_000_000.0,
                    total as f64 / 1_000_000_000.0,
                    pct
                ));
            }

            if let Some(pkg_count) = ext.package_count {
                output.push_str(&format!("  Packages Installed: {}\n", pkg_count));
            }
        }

        // Tool Summary
        output.push_str(&subheader("Tool Summary"));
        output.push_str(&format!(
            "  {}✓{} Healthy: {}  ",
            colors::GREEN,
            colors::RESET,
            self.summary.healthy
        ));
        output.push_str(&format!(
            "{}⚠{} Outdated: {}  ",
            colors::YELLOW,
            colors::RESET,
            self.summary.outdated
        ));
        output.push_str(&format!(
            "{}✗{} Missing: {}  ",
            colors::RED,
            colors::RESET,
            self.summary.missing
        ));
        output.push_str(&format!(
            "{}?{} Unknown: {}\n",
            colors::CYAN,
            colors::RESET,
            self.summary.unknown
        ));

        // PATH Analysis
        if !self.base.path_checks.is_empty() {
            output.push_str(&subheader("PATH Analysis"));
            for check in &self.base.path_checks {
                let (icon, color) = match check.status {
                    PathStatus::Ok => (icons::OK, colors::GREEN),
                    PathStatus::Missing => (icons::ERROR, colors::RED),
                    PathStatus::NotInPath => (icons::WARN, colors::YELLOW),
                };
                let status_msg = if check.in_path {
                    "in PATH"
                } else {
                    "not in PATH"
                };
                output.push_str(&format!(
                    "  {}{}{} {} - {}\n",
                    color,
                    icon,
                    colors::RESET,
                    check.path,
                    status_msg
                ));
            }
        }

        // Tool Health
        if !self.base.tools.is_empty() {
            output.push_str(&subheader("Tool Health"));
            for tool in &self.base.tools {
                let (icon, color) = match tool.status {
                    ToolStatus::Ok => (icons::OK, colors::GREEN),
                    ToolStatus::Outdated => (icons::WARN, colors::YELLOW),
                    ToolStatus::NotInstalled => (icons::ERROR, colors::RED),
                    ToolStatus::Unknown => (icons::INFO, colors::CYAN),
                };

                let installed_str = tool
                    .installed
                    .as_ref()
                    .map(|v| format!(" (installed: {})", v))
                    .unwrap_or_else(|| " - not found".to_string());

                let status_msg = match tool.status {
                    ToolStatus::Ok => "satisfies requirement",
                    ToolStatus::Outdated => "outdated",
                    ToolStatus::NotInstalled => "not installed",
                    ToolStatus::Unknown => "unknown tool",
                };

                output.push_str(&format!(
                    "  {}{}{} {} {}{} - {}\n",
                    color,
                    icon,
                    colors::RESET,
                    tool.name,
                    tool.required,
                    installed_str,
                    status_msg
                ));
            }
        }

        // Hooks Status
        if !self.base.hooks.is_empty() {
            output.push_str(&subheader("Hooks Status"));
            for hook in &self.base.hooks {
                let (icon, color) = if hook.active {
                    (icons::OK, colors::GREEN)
                } else {
                    (icons::WARN, colors::YELLOW)
                };
                output.push_str(&format!(
                    "  {}{}{} {}: {}\n",
                    color,
                    icon,
                    colors::RESET,
                    hook.name,
                    hook.description
                ));
                if let Some(ref issue) = hook.issue {
                    output.push_str(&format!(
                        "      {}{}{}\n",
                        colors::DIM,
                        issue,
                        colors::RESET
                    ));
                }
            }
        }

        // Recommendations
        if !self.base.recommendations.is_empty() {
            output.push_str(&subheader("Recommendations"));
            for (i, rec) in self.base.recommendations.iter().enumerate() {
                let color = match rec.severity {
                    RecommendationSeverity::Error => colors::RED,
                    RecommendationSeverity::Warning => colors::YELLOW,
                    RecommendationSeverity::Info => colors::CYAN,
                };
                output.push_str(&format!(
                    "  {}{}. {}{}\n",
                    color,
                    i + 1,
                    rec.message,
                    colors::RESET
                ));
                if let Some(ref fix) = rec.fix {
                    output.push_str(&format!("     Fix: {}\n", fix));
                }
            }
        }

        output
    }

    fn exit_code(&self) -> ExitCode {
        self.base.exit_code()
    }
}

/// Run the doctor command with extended metrics
/// Category-filtered `--extended` dashboard. The `--check` filter narrows
/// the base sections (path / tools / hooks); the system-metrics panel and
/// tool summary are always part of the extended dashboard. (The
/// unfiltered form is just `run_doctor_extended_filtered(.., None)`.)
pub fn run_doctor_extended_filtered(
    config: Option<&Config>,
    specific_tools: Option<Vec<String>>,
    categories: Option<&[DoctorCategory]>,
) -> DoctorResultExtended {
    let base = run_doctor_filtered(config, specific_tools, categories);

    // Collect extended metrics
    let extended = collect_extended_metrics();

    // Calculate summary
    let summary = ToolSummary {
        total: base.tools.len(),
        healthy: base
            .tools
            .iter()
            .filter(|t| t.status == ToolStatus::Ok)
            .count(),
        outdated: base
            .tools
            .iter()
            .filter(|t| t.status == ToolStatus::Outdated)
            .count(),
        missing: base
            .tools
            .iter()
            .filter(|t| t.status == ToolStatus::NotInstalled)
            .count(),
        unknown: base
            .tools
            .iter()
            .filter(|t| t.status == ToolStatus::Unknown)
            .count(),
    };

    DoctorResultExtended {
        base,
        extended: Some(extended),
        summary,
    }
}

/// Collect extended system metrics
fn collect_extended_metrics() -> ExtendedMetrics {
    let mut metrics = ExtendedMetrics::default();

    // Get uptime
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("sysctl")
            .args(["-n", "kern.boottime"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                // Parse: { sec = 1234567890, usec = 0 }
                if let Some(sec_str) = text.split("sec = ").nth(1) {
                    if let Some(sec) = sec_str.split(',').next() {
                        if let Ok(boot_time) = sec.trim().parse::<u64>() {
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs())
                                .unwrap_or(0);
                            metrics.uptime_secs = Some(now.saturating_sub(boot_time));
                        }
                    }
                }
            }
        }

        // Get load averages
        if let Ok(output) = std::process::Command::new("sysctl")
            .args(["-n", "vm.loadavg"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                // Parse: { 1.23 2.34 3.45 }
                let parts: Vec<f64> = text
                    .trim()
                    .trim_start_matches('{')
                    .trim_end_matches('}')
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if parts.len() >= 3 {
                    metrics.load_avg = Some((parts[0], parts[1], parts[2]));
                }
            }
        }

        // Get memory info
        if let Ok(output) = std::process::Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                if let Ok(total) = text.trim().parse::<u64>() {
                    metrics.memory_total = Some(total);

                    // Get page size and memory stats
                    if let (Ok(ps_output), Ok(vm_output)) = (
                        std::process::Command::new("sysctl")
                            .args(["-n", "hw.pagesize"])
                            .output(),
                        std::process::Command::new("vm_stat").output(),
                    ) {
                        if let (Ok(ps_text), Ok(vm_text)) = (
                            String::from_utf8(ps_output.stdout),
                            String::from_utf8(vm_output.stdout),
                        ) {
                            if let Ok(page_size) = ps_text.trim().parse::<u64>() {
                                let mut free_pages = 0u64;
                                let mut inactive_pages = 0u64;

                                for line in vm_text.lines() {
                                    if line.contains("Pages free") {
                                        if let Some(num) = line.split(':').nth(1) {
                                            free_pages = num
                                                .trim()
                                                .trim_end_matches('.')
                                                .parse()
                                                .unwrap_or(0);
                                        }
                                    } else if line.contains("Pages inactive") {
                                        if let Some(num) = line.split(':').nth(1) {
                                            inactive_pages = num
                                                .trim()
                                                .trim_end_matches('.')
                                                .parse()
                                                .unwrap_or(0);
                                        }
                                    }
                                }

                                let available = (free_pages + inactive_pages) * page_size;
                                metrics.memory_used = Some(total.saturating_sub(available));
                            }
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Get uptime
        if let Ok(content) = std::fs::read_to_string("/proc/uptime") {
            if let Some(uptime_str) = content.split_whitespace().next() {
                if let Ok(uptime) = uptime_str.parse::<f64>() {
                    metrics.uptime_secs = Some(uptime as u64);
                }
            }
        }

        // Get load averages
        if let Ok(content) = std::fs::read_to_string("/proc/loadavg") {
            let parts: Vec<f64> = content
                .split_whitespace()
                .take(3)
                .filter_map(|s| s.parse().ok())
                .collect();
            if parts.len() >= 3 {
                metrics.load_avg = Some((parts[0], parts[1], parts[2]));
            }
        }

        // Get memory info
        if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
            let mut total = 0u64;
            let mut available = 0u64;

            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(kb) = line.split_whitespace().nth(1) {
                        total = kb.parse::<u64>().unwrap_or(0) * 1024;
                    }
                } else if line.starts_with("MemAvailable:") {
                    if let Some(kb) = line.split_whitespace().nth(1) {
                        available = kb.parse::<u64>().unwrap_or(0) * 1024;
                    }
                }
            }

            if total > 0 {
                metrics.memory_total = Some(total);
                metrics.memory_used = Some(total.saturating_sub(available));
            }
        }
    }

    // Get disk info (cross-platform)
    if let Ok(output) = std::process::Command::new("df").args(["-k", "/"]).output() {
        if let Ok(text) = String::from_utf8(output.stdout) {
            if let Some(line) = text.lines().nth(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    if let (Ok(total), Ok(avail)) =
                        (parts[1].parse::<u64>(), parts[3].parse::<u64>())
                    {
                        metrics.disk_total = Some(total * 1024);
                        metrics.disk_available = Some(avail * 1024);
                    }
                }
            }
        }
    }

    // Get package count
    #[cfg(target_os = "macos")]
    {
        if has("brew") {
            if let Ok(output) = std::process::Command::new("brew")
                .args(["list", "--formula"])
                .output()
            {
                if let Ok(text) = String::from_utf8(output.stdout) {
                    metrics.package_count = Some(text.lines().count());
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if has("dpkg") {
            if let Ok(output) = std::process::Command::new("dpkg")
                .args(["--get-selections"])
                .output()
            {
                if let Ok(text) = String::from_utf8(output.stdout) {
                    metrics.package_count =
                        Some(text.lines().filter(|l| l.contains("install")).count());
                }
            }
        } else if has("rpm") {
            if let Ok(output) = std::process::Command::new("rpm").args(["-qa"]).output() {
                if let Ok(text) = String::from_utf8(output.stdout) {
                    metrics.package_count = Some(text.lines().count());
                }
            }
        }
    }

    metrics
}

/// Export doctor result as markdown report
pub fn export_report(result: &DoctorResultExtended, path: &str) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;

    writeln!(file, "# Jarvy Health Report")?;
    writeln!(file)?;
    writeln!(file, "Generated: {}", chrono_lite_now())?;
    writeln!(file)?;

    // System Information
    writeln!(file, "## System Information")?;
    writeln!(file)?;
    writeln!(file, "| Property | Value |")?;
    writeln!(file, "|----------|-------|")?;
    writeln!(
        file,
        "| OS | {} {} |",
        result.base.system.os, result.base.system.os_version
    )?;
    writeln!(file, "| Architecture | {} |", result.base.system.arch)?;
    writeln!(file, "| Shell | {} |", result.base.system.shell)?;
    if let Some(ref pm) = result.base.system.package_manager {
        writeln!(file, "| Package Manager | {} |", pm)?;
    }
    writeln!(file)?;

    // Extended metrics
    if let Some(ref ext) = result.extended {
        writeln!(file, "## System Metrics")?;
        writeln!(file)?;
        writeln!(file, "| Metric | Value |")?;
        writeln!(file, "|--------|-------|")?;

        if let Some(uptime) = ext.uptime_secs {
            let days = uptime / 86400;
            let hours = (uptime % 86400) / 3600;
            writeln!(file, "| Uptime | {}d {}h |", days, hours)?;
        }

        if let Some((l1, l5, l15)) = ext.load_avg {
            writeln!(file, "| Load Average | {:.2}, {:.2}, {:.2} |", l1, l5, l15)?;
        }

        if let (Some(total), Some(used)) = (ext.memory_total, ext.memory_used) {
            let pct = (used as f64 / total as f64) * 100.0;
            writeln!(
                file,
                "| Memory | {:.1} GB / {:.1} GB ({:.0}%) |",
                used as f64 / 1_000_000_000.0,
                total as f64 / 1_000_000_000.0,
                pct
            )?;
        }

        if let (Some(total), Some(avail)) = (ext.disk_total, ext.disk_available) {
            let used = total - avail;
            let pct = (used as f64 / total as f64) * 100.0;
            writeln!(
                file,
                "| Disk | {:.1} GB / {:.1} GB ({:.0}%) |",
                used as f64 / 1_000_000_000.0,
                total as f64 / 1_000_000_000.0,
                pct
            )?;
        }

        if let Some(count) = ext.package_count {
            writeln!(file, "| Packages | {} |", count)?;
        }
        writeln!(file)?;
    }

    // Tool Summary
    writeln!(file, "## Tool Summary")?;
    writeln!(file)?;
    writeln!(file, "- **Total**: {}", result.summary.total)?;
    writeln!(file, "- **Healthy**: {} ✅", result.summary.healthy)?;
    writeln!(file, "- **Outdated**: {} ⚠️", result.summary.outdated)?;
    writeln!(file, "- **Missing**: {} ❌", result.summary.missing)?;
    writeln!(file, "- **Unknown**: {} ❓", result.summary.unknown)?;
    writeln!(file)?;

    // Tool Details
    if !result.base.tools.is_empty() {
        writeln!(file, "## Tool Details")?;
        writeln!(file)?;
        writeln!(file, "| Tool | Required | Installed | Status |")?;
        writeln!(file, "|------|----------|-----------|--------|")?;

        for tool in &result.base.tools {
            let installed = tool.installed.as_deref().unwrap_or("-");
            let status = match tool.status {
                ToolStatus::Ok => "✅ OK",
                ToolStatus::Outdated => "⚠️ Outdated",
                ToolStatus::NotInstalled => "❌ Missing",
                ToolStatus::Unknown => "❓ Unknown",
            };
            writeln!(
                file,
                "| {} | {} | {} | {} |",
                tool.name, tool.required, installed, status
            )?;
        }
        writeln!(file)?;
    }

    // Recommendations
    if !result.base.recommendations.is_empty() {
        writeln!(file, "## Recommendations")?;
        writeln!(file)?;

        for (i, rec) in result.base.recommendations.iter().enumerate() {
            let severity = match rec.severity {
                RecommendationSeverity::Error => "🔴",
                RecommendationSeverity::Warning => "🟡",
                RecommendationSeverity::Info => "🔵",
            };
            writeln!(file, "{}. {} {}", i + 1, severity, rec.message)?;
            if let Some(ref fix) = rec.fix {
                writeln!(file, "   - **Fix**: `{}`", fix)?;
            }
        }
        writeln!(file)?;
    }

    Ok(())
}

/// Simple timestamp without chrono crate
fn chrono_lite_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Convert to human-readable (simplified)
    let days_since_1970 = secs / 86400;
    // Approximate year calculation (not accounting for leap years precisely)
    let years = 1970 + (days_since_1970 / 365);
    let remaining_days = days_since_1970 % 365;
    let month = remaining_days / 30 + 1;
    let day = remaining_days % 30 + 1;
    let hour = (secs % 86400) / 3600;
    let min = (secs % 3600) / 60;

    format!(
        "{}-{:02}-{:02} {:02}:{:02} UTC",
        years, month, day, hour, min
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_system_info() {
        let info = collect_system_info();
        assert!(!info.os.is_empty());
        assert!(!info.arch.is_empty());
    }

    #[test]
    fn doctor_category_parse_list_dedups_and_normalizes() {
        assert_eq!(
            DoctorCategory::parse_list("tools, path , tools").unwrap(),
            vec![DoctorCategory::Tools, DoctorCategory::Path]
        );
        // Singular/plural aliases and case-insensitivity.
        assert_eq!(
            DoctorCategory::parse_list("HOOK").unwrap(),
            vec![DoctorCategory::Hooks]
        );
    }

    #[test]
    fn doctor_category_parse_list_rejects_unknown_and_empty() {
        assert!(DoctorCategory::parse_list("network").is_err());
        assert!(DoctorCategory::parse_list("").is_err());
        assert!(DoctorCategory::parse_list(" , ").is_err());
    }

    #[test]
    fn doctor_filter_skips_unselected_sections() {
        // `--check tools` must not populate path/hook sections.
        let result = run_doctor_filtered(
            None,
            Some(vec!["git".into()]),
            Some(&[DoctorCategory::Tools]),
        );
        assert!(result.path_checks.is_empty(), "path must be skipped");
        assert!(result.hooks.is_empty(), "hooks must be skipped");
        assert!(!result.tools.is_empty(), "tools must be checked");
    }

    #[test]
    fn test_extract_version() {
        assert_eq!(
            extract_version("git version 2.43.0"),
            Some("2.43.0".to_string())
        );
        assert_eq!(extract_version("v20.11.0"), Some("20.11.0".to_string()));
        assert_eq!(extract_version("Python 3.12.1"), Some("3.12.1".to_string()));
        assert_eq!(extract_version("1.75.0"), Some("1.75.0".to_string()));
    }

    #[test]
    fn test_path_status_serialization() {
        let check = PathCheck {
            path: "/test".to_string(),
            status: PathStatus::Ok,
            in_path: true,
        };
        let json = serde_json::to_string(&check).unwrap();
        assert!(json.contains("\"status\":\"ok\""));
    }

    #[test]
    fn test_tool_status_serialization() {
        let health = ToolHealth {
            name: "git".to_string(),
            required: "latest".to_string(),
            installed: Some("2.43.0".to_string()),
            status: ToolStatus::Ok,
            path: Some("/usr/bin/git".to_string()),
            dependencies: None,
        };
        let json = serde_json::to_string(&health).unwrap();
        assert!(json.contains("\"status\":\"ok\""));
    }

    #[test]
    fn test_dependency_info_serialization() {
        let deps = DependencyInfo {
            satisfied: true,
            satisfied_by: Some("docker".to_string()),
            missing_required: vec![],
            missing_flexible: None,
            will_install: None,
        };
        let json = serde_json::to_string(&deps).unwrap();
        assert!(json.contains("\"satisfied\":true"));
        assert!(json.contains("\"satisfied_by\":\"docker\""));
    }

    #[test]
    fn test_dependency_info_missing_flexible() {
        let deps = DependencyInfo {
            satisfied: false,
            satisfied_by: None,
            missing_required: vec![],
            missing_flexible: Some(FlexibleDepInfo {
                options: vec!["docker".to_string(), "podman".to_string()],
                suggestion: Some("docker".to_string()),
            }),
            will_install: None,
        };
        let json = serde_json::to_string(&deps).unwrap();
        assert!(json.contains("\"satisfied\":false"));
        assert!(json.contains("\"missing_flexible\""));
        assert!(json.contains("\"options\""));
    }

    #[test]
    fn test_doctor_result_exit_codes() {
        let result = DoctorResult {
            system: collect_system_info(),
            path_checks: vec![],
            tools: vec![],
            hooks: vec![],
            recommendations: vec![],
            exit_code: 0,
        };
        assert_eq!(result.exit_code(), ExitCode::Ok);

        let result_warn = DoctorResult {
            exit_code: 1,
            ..result.clone()
        };
        assert_eq!(result_warn.exit_code(), ExitCode::Warning);

        let result_err = DoctorResult {
            exit_code: 2,
            ..result
        };
        assert_eq!(result_err.exit_code(), ExitCode::Error);
    }
}
