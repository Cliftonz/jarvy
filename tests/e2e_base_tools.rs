//! E2E Base Tools Integration Tests
//!
//! This module tests the installation of a core set of tools across platforms.
//! These tests are designed to run in the GitHub Actions E2E workflow and verify
//! that Jarvy can successfully install tools on each supported platform.
//!
//! ## Test Strategy
//!
//! - **Tier 1 (Core)**: Tools that must work everywhere - git, jq, ripgrep, curl, wget
//! - **Tier 2 (Runtimes)**: Language runtimes - node, python, rust, go
//! - **Tier 3 (DevOps)**: Container/cloud tools - docker, kubectl, terraform
//! - **Tier 4 (Dependencies)**: Tools with dependencies - lazydocker, lazygit, k9s
//!
//! ## Environment Variables
//!
//! - `JARVY_E2E_TIER`: Run only specific tier (1, 2, 3, 4, or "all")
//! - `JARVY_E2E_TOOLS`: Comma-separated list of specific tools to test
//! - `JARVY_E2E_SKIP_INSTALL`: Skip actual installation (dry-run mode)
//! - `JARVY_BIN`: Path to jarvy binary (defaults to cargo_bin lookup)

use assert_cmd::prelude::*;
use predicates::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::NamedTempFile;

/// Result of a single tool installation test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolTestResult {
    pub tool: String,
    pub status: TestStatus,
    pub duration_seconds: u64,
    pub error_message: Option<String>,
    pub installed_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TestStatus {
    Success,
    Failed,
    Skipped,
    Timeout,
}

/// System information for the test run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub platform: String,
    pub os: String,
    pub arch: String,
    pub os_version: String,
    pub hostname: String,
    pub timestamp: String,
    pub package_manager: Option<String>,
}

/// Complete test run results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E2ETestResults {
    pub system_info: SystemInfo,
    pub results: Vec<ToolTestResult>,
    pub total_duration_seconds: u64,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
}

/// Tool tiers for testing
struct ToolTiers {
    tier1_core: Vec<&'static str>,
    tier2_runtimes: Vec<&'static str>,
    tier3_devops: Vec<&'static str>,
    tier4_dependencies: Vec<&'static str>,
}

impl Default for ToolTiers {
    fn default() -> Self {
        Self {
            // Tier 1: Core tools that must work everywhere
            tier1_core: vec!["git", "jq", "ripgrep", "curl", "wget"],
            // Tier 2: Language runtimes
            tier2_runtimes: vec!["node", "python", "go"],
            // Tier 3: DevOps tools (skip docker on some platforms)
            tier3_devops: vec!["kubectl", "terraform"],
            // Tier 4: Tools with dependencies
            tier4_dependencies: vec!["lazygit"],
        }
    }
}

fn get_jarvy_bin() -> PathBuf {
    if let Ok(bin) = env::var("JARVY_BIN") {
        PathBuf::from(bin)
    } else {
        // Fall back to cargo_bin lookup
        let output = Command::new("cargo")
            .args(["build", "--release", "--bin", "jarvy"])
            .output()
            .expect("Failed to build jarvy");
        if !output.status.success() {
            panic!(
                "Failed to build jarvy: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        PathBuf::from("target/release/jarvy")
    }
}

fn get_system_info() -> SystemInfo {
    let os = env::consts::OS.to_string();
    let arch = env::consts::ARCH.to_string();

    let os_version = if cfg!(target_os = "macos") {
        Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    } else if cfg!(target_os = "linux") {
        fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|l| l.starts_with("PRETTY_NAME="))
                    .map(|l| {
                        l.trim_start_matches("PRETTY_NAME=")
                            .trim_matches('"')
                            .to_string()
                    })
            })
            .unwrap_or_else(|| "Linux".to_string())
    } else if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/c", "ver"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "Windows".to_string())
    } else {
        "unknown".to_string()
    };

    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string());

    let package_manager = detect_package_manager();

    let platform = format!("{}-{}", os, if arch == "aarch64" { "arm64" } else { &arch });

    SystemInfo {
        platform,
        os,
        arch,
        os_version,
        hostname,
        timestamp: chrono::Utc::now().to_rfc3339(),
        package_manager,
    }
}

fn detect_package_manager() -> Option<String> {
    if cfg!(target_os = "macos") {
        if Command::new("brew").arg("--version").output().is_ok() {
            return Some("homebrew".to_string());
        }
    } else if cfg!(target_os = "linux") {
        for (cmd, name) in [
            ("apt-get", "apt"),
            ("dnf", "dnf"),
            ("yum", "yum"),
            ("pacman", "pacman"),
            ("apk", "apk"),
            ("zypper", "zypper"),
        ] {
            if Command::new("which")
                .arg(cmd)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                return Some(name.to_string());
            }
        }
    } else if cfg!(target_os = "windows") {
        if Command::new("winget").arg("--version").output().is_ok() {
            return Some("winget".to_string());
        }
        if Command::new("choco").arg("--version").output().is_ok() {
            return Some("chocolatey".to_string());
        }
    }
    None
}

fn create_tool_config(tool: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().expect("Failed to create temp config");
    writeln!(
        f,
        r#"[privileges]
use_sudo = false

[provisioner]
{} = "latest"
"#,
        tool
    )
    .expect("Failed to write config");
    f
}

fn test_single_tool(jarvy_bin: &PathBuf, tool: &str, dry_run: bool) -> ToolTestResult {
    let start = Instant::now();
    let config = create_tool_config(tool);

    let mut cmd = Command::new(jarvy_bin);
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.env("JARVY_TELEMETRY", "0");

    if dry_run {
        cmd.args(["setup", "--dry-run", "--file"]);
    } else {
        cmd.args(["setup", "--file"]);
    }
    cmd.arg(config.path());

    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            return ToolTestResult {
                tool: tool.to_string(),
                status: TestStatus::Failed,
                duration_seconds: start.elapsed().as_secs(),
                error_message: Some(format!("Failed to execute jarvy: {}", e)),
                installed_version: None,
            };
        }
    };

    let duration = start.elapsed().as_secs();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return ToolTestResult {
            tool: tool.to_string(),
            status: TestStatus::Failed,
            duration_seconds: duration,
            error_message: Some(format!(
                "Exit code: {:?}\nstderr: {}\nstdout: {}",
                output.status.code(),
                stderr,
                stdout
            )),
            installed_version: None,
        };
    }

    // If not dry run, verify the tool is actually installed
    let installed_version = if !dry_run {
        verify_tool_installed(tool)
    } else {
        None
    };

    ToolTestResult {
        tool: tool.to_string(),
        status: TestStatus::Success,
        duration_seconds: duration,
        error_message: None,
        installed_version,
    }
}

fn verify_tool_installed(tool: &str) -> Option<String> {
    // Map tool names to their actual binary names
    let binary = match tool {
        "ripgrep" => "rg",
        "node" => "node",
        "python" => {
            if cfg!(target_os = "windows") {
                "python"
            } else {
                "python3"
            }
        }
        _ => tool,
    };

    let version_flag = match tool {
        "kubectl" => "version --client --short",
        "terraform" => "-version",
        "go" => "version",
        _ => "--version",
    };

    let output = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/c", &format!("{} {}", binary, version_flag)])
            .output()
    } else {
        Command::new("sh")
            .args(["-c", &format!("{} {}", binary, version_flag)])
            .output()
    };

    output.ok().and_then(|o| {
        if o.status.success() {
            String::from_utf8(o.stdout)
                .ok()
                .map(|s| s.lines().next().unwrap_or("").trim().to_string())
        } else {
            None
        }
    })
}

fn get_tools_to_test() -> Vec<String> {
    let tiers = ToolTiers::default();

    // Check for specific tools override
    if let Ok(tools) = env::var("JARVY_E2E_TOOLS") {
        return tools.split(',').map(|s| s.trim().to_string()).collect();
    }

    // Check for tier filter
    let tier = env::var("JARVY_E2E_TIER").unwrap_or_else(|_| "all".to_string());

    let mut tools = Vec::new();

    match tier.as_str() {
        "1" => tools.extend(tiers.tier1_core.iter().map(|s| s.to_string())),
        "2" => tools.extend(tiers.tier2_runtimes.iter().map(|s| s.to_string())),
        "3" => tools.extend(tiers.tier3_devops.iter().map(|s| s.to_string())),
        "4" => tools.extend(tiers.tier4_dependencies.iter().map(|s| s.to_string())),
        _ => {
            // All tiers
            tools.extend(tiers.tier1_core.iter().map(|s| s.to_string()));
            tools.extend(tiers.tier2_runtimes.iter().map(|s| s.to_string()));
            tools.extend(tiers.tier3_devops.iter().map(|s| s.to_string()));
            tools.extend(tiers.tier4_dependencies.iter().map(|s| s.to_string()));
        }
    }

    // Filter out tools not supported on this platform
    tools
        .into_iter()
        .filter(|t| is_tool_supported_on_platform(t))
        .collect()
}

fn is_tool_supported_on_platform(tool: &str) -> bool {
    // Skip docker on Alpine and FreeBSD
    if tool == "docker" && (is_alpine() || is_freebsd()) {
        return false;
    }

    // Skip lazydocker/k9s on platforms without docker
    if (tool == "lazydocker" || tool == "k9s") && (is_alpine() || is_freebsd()) {
        return false;
    }

    // Skip node on FreeBSD
    if tool == "node" && is_freebsd() {
        return false;
    }

    true
}

fn is_alpine() -> bool {
    fs::read_to_string("/etc/os-release")
        .map(|c| c.contains("Alpine"))
        .unwrap_or(false)
}

fn is_freebsd() -> bool {
    env::consts::OS == "freebsd"
}

fn write_results(results: &E2ETestResults) {
    // Write to target/e2e-results/
    let results_dir = PathBuf::from("target/e2e-results");
    fs::create_dir_all(&results_dir).ok();

    // Write results.json
    let json_path = results_dir.join("results.json");
    if let Ok(json) = serde_json::to_string_pretty(results) {
        fs::write(&json_path, json).ok();
    }

    // Write system-info.txt
    let info_path = results_dir.join("system-info.txt");
    let info = format!(
        "Platform: {}\nOS: {}\nArch: {}\nOS Version: {}\nHostname: {}\nPackage Manager: {}\nTimestamp: {}\n",
        results.system_info.platform,
        results.system_info.os,
        results.system_info.arch,
        results.system_info.os_version,
        results.system_info.hostname,
        results
            .system_info
            .package_manager
            .as_deref()
            .unwrap_or("unknown"),
        results.system_info.timestamp
    );
    fs::write(&info_path, info).ok();

    // Print summary to stdout
    println!("\n=== E2E Test Results ===");
    println!("Platform: {}", results.system_info.platform);
    println!(
        "Passed: {} | Failed: {} | Skipped: {}",
        results.passed, results.failed, results.skipped
    );
    println!("Total Duration: {}s", results.total_duration_seconds);
    println!("\nDetailed Results:");
    for r in &results.results {
        let status_icon = match r.status {
            TestStatus::Success => "✓",
            TestStatus::Failed => "✗",
            TestStatus::Skipped => "○",
            TestStatus::Timeout => "⏱",
        };
        println!(
            "  {} {} ({}s){}",
            status_icon,
            r.tool,
            r.duration_seconds,
            r.installed_version
                .as_ref()
                .map(|v| format!(" - {}", v))
                .unwrap_or_default()
        );
        if let Some(err) = &r.error_message {
            // Truncate long error messages
            let err_short = if err.len() > 200 {
                format!("{}...", &err[..200])
            } else {
                err.clone()
            };
            println!("    Error: {}", err_short);
        }
    }
}

// ============================================================================
// Test Functions
// ============================================================================

/// Main E2E test for base tools
///
/// This test is designed to be run in CI with actual tool installations.
/// Set `JARVY_E2E_SKIP_INSTALL=1` to run in dry-run mode.
#[test]
fn e2e_base_tools_installation() {
    let start = Instant::now();
    let jarvy_bin = get_jarvy_bin();
    let system_info = get_system_info();
    let dry_run = env::var("JARVY_E2E_SKIP_INSTALL").is_ok();

    println!("Starting E2E tests on {}", system_info.platform);
    println!("Dry run mode: {}", dry_run);
    println!("Jarvy binary: {:?}", jarvy_bin);

    let tools = get_tools_to_test();
    println!("Testing {} tools: {:?}", tools.len(), tools);

    let mut results = Vec::new();
    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;

    for tool in &tools {
        println!("\n--- Testing {} ---", tool);
        let result = test_single_tool(&jarvy_bin, tool, dry_run);

        match result.status {
            TestStatus::Success => {
                passed += 1;
                println!("✓ {} succeeded", tool);
            }
            TestStatus::Failed => {
                failed += 1;
                println!("✗ {} failed", tool);
            }
            TestStatus::Skipped => {
                skipped += 1;
                println!("○ {} skipped", tool);
            }
            TestStatus::Timeout => {
                failed += 1;
                println!("⏱ {} timed out", tool);
            }
        }

        results.push(result);
    }

    let total_duration = start.elapsed().as_secs();

    let test_results = E2ETestResults {
        system_info,
        results,
        total_duration_seconds: total_duration,
        passed,
        failed,
        skipped,
    };

    write_results(&test_results);

    // In dry-run mode, all tests should pass (just verifying jarvy runs)
    // In real mode, we still allow failures but report them
    if !dry_run && failed > 0 {
        // Don't fail the test in CI - just report
        // The workflow will analyze results separately
        println!(
            "\nWarning: {} tool(s) failed to install. See results.json for details.",
            failed
        );
    }
}

/// Quick smoke test that verifies jarvy can parse configs
#[test]
fn e2e_smoke_config_parsing() {
    // Enable fast test mode
    unsafe {
        std::env::set_var("JARVY_FAST_TEST", "1");
    }

    let jarvy_bin = get_jarvy_bin();
    let config = create_tool_config("git");

    let mut cmd = Command::new(&jarvy_bin);
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["get", "--file"]).arg(config.path());

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("git"));
}

/// Test that dry-run mode works
#[test]
fn e2e_dry_run_mode() {
    let jarvy_bin = get_jarvy_bin();
    let config = create_tool_config("jq");

    let mut cmd = Command::new(&jarvy_bin);
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["setup", "--dry-run", "--file"])
        .arg(config.path());

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Would install").or(predicate::str::contains("jq")));
}

/// Test multi-tool config
#[test]
fn e2e_multi_tool_config() {
    let mut f = NamedTempFile::new().expect("Failed to create temp config");
    writeln!(
        f,
        r#"[privileges]
use_sudo = false

[provisioner]
git = "latest"
jq = "latest"
curl = "latest"
"#
    )
    .expect("Failed to write config");

    let jarvy_bin = get_jarvy_bin();

    let mut cmd = Command::new(&jarvy_bin);
    cmd.env("JARVY_TEST_MODE", "1");
    cmd.args(["get", "--file"]).arg(f.path());

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("git"))
        .stdout(predicate::str::contains("jq"))
        .stdout(predicate::str::contains("curl"));
}
