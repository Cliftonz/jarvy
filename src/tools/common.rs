use std::process::{Command, Output};
use std::sync::OnceLock;

use crate::network::config::NetworkConfig;
use crate::network::propagate::apply_network_config;

#[derive(thiserror::Error, Debug)]
pub enum InstallError {
    #[error("unsupported platform")]
    Unsupported,
    #[error("prerequisite missing: {0}")]
    Prereq(&'static str),
    #[error("invalid permissions: {0}")]
    InvalidPermissions(&'static str),
    #[error("command failed: {cmd} (code: {code:?})\n{stderr}")]
    CommandFailed {
        cmd: String,
        code: Option<i32>,
        stderr: String,
    },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(&'static str),
}

// OS enum for config keys and runtime resolution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Linux,
    Macos,
    Windows,
    Bsd,
}

// Determine the current OS as our enum
pub fn current_os() -> Os {
    #[cfg(target_os = "linux")]
    {
        Os::Linux
    }
    #[cfg(target_os = "macos")]
    {
        Os::Macos
    }
    #[cfg(target_os = "windows")]
    {
        Os::Windows
    }
    #[cfg(target_os = "freebsd")]
    {
        Os::Bsd
    }
}

// Global default for whether to use sudo on POSIX installs. Can be set from Config in main.
// None means: auto-detect per operation (try without sudo, then with if available).
static USE_SUDO_DEFAULT: OnceLock<Option<bool>> = OnceLock::new();

pub fn set_default_use_sudo(val: Option<bool>) {
    let _ = USE_SUDO_DEFAULT.set(val);
}

pub fn default_use_sudo() -> Option<bool> {
    if let Some(v) = USE_SUDO_DEFAULT.get() {
        *v
    } else {
        // Unset -> auto mode
        None
    }
}

pub fn run(cmd: &str, args: &[&str]) -> Result<Output, InstallError> {
    // Fast, deterministic tests: allow skipping external command execution.
    // Integration tests can opt-in via JARVY_FAST_TEST; unit tests default to skip unless explicitly overridden.
    if std::env::var_os("JARVY_FAST_TEST").is_some() {
        return Err(InstallError::Prereq(
            "skipped external command in fast test mode",
        ));
    }
    #[cfg(test)]
    {
        if std::env::var_os("JARVY_RUN_EXTERNAL_CMDS_IN_TEST").is_none() {
            return Err(InstallError::Prereq(
                "external commands disabled during unit tests",
            ));
        }
    }

    let out = Command::new(cmd).args(args).output().map_err(|e| {
        use std::io::ErrorKind::*;
        match e.kind() {
            NotFound => InstallError::Prereq("required command not found on PATH"),
            PermissionDenied => {
                InstallError::InvalidPermissions("operation requires elevated privileges")
            }
            _ => InstallError::Io(e),
        }
    })?;

    if !out.status.success() {
        // Try to capture stderr for easier diagnostics.
        return Err(InstallError::CommandFailed {
            cmd: cmd.to_string(),
            code: out.status.code(),
            stderr: String::from_utf8_lossy(&out.stderr).into(),
        });
    }
    Ok(out)
}

/// Run a command with network/proxy configuration applied.
///
/// This variant applies HTTP_PROXY, HTTPS_PROXY, NO_PROXY, and CA bundle
/// environment variables to the spawned process based on the NetworkConfig.
pub fn run_with_network(
    cmd: &str,
    args: &[&str],
    network: Option<&NetworkConfig>,
    tool_name: &str,
) -> Result<Output, InstallError> {
    // Fast, deterministic tests: allow skipping external command execution.
    if std::env::var_os("JARVY_FAST_TEST").is_some() {
        return Err(InstallError::Prereq(
            "skipped external command in fast test mode",
        ));
    }
    #[cfg(test)]
    {
        if std::env::var_os("JARVY_RUN_EXTERNAL_CMDS_IN_TEST").is_none() {
            return Err(InstallError::Prereq(
                "external commands disabled during unit tests",
            ));
        }
    }

    let mut command = Command::new(cmd);
    command.args(args);

    // Apply network/proxy configuration if provided
    if let Some(net_config) = network {
        apply_network_config(&mut command, net_config, tool_name);
    }

    let out = command.output().map_err(|e| {
        use std::io::ErrorKind::*;
        match e.kind() {
            NotFound => InstallError::Prereq("required command not found on PATH"),
            PermissionDenied => {
                InstallError::InvalidPermissions("operation requires elevated privileges")
            }
            _ => InstallError::Io(e),
        }
    })?;

    if !out.status.success() {
        return Err(InstallError::CommandFailed {
            cmd: cmd.to_string(),
            code: out.status.code(),
            stderr: String::from_utf8_lossy(&out.stderr).into(),
        });
    }
    Ok(out)
}

// Run a command, prefixing with sudo if configured and applicable (non-Windows)
pub fn run_maybe_sudo(use_sudo: bool, cmd: &str, args: &[&str]) -> Result<Output, InstallError> {
    match current_os() {
        Os::Windows => run(cmd, args),
        Os::Linux | Os::Macos | Os::Bsd => {
            if use_sudo {
                // sudo <cmd> <args...>
                let mut all = Vec::with_capacity(1 + args.len());
                all.push(cmd);
                all.extend_from_slice(args);
                run("sudo", &all)
            } else {
                run(cmd, args)
            }
        }
    }
}

/// Run a command with sudo and network/proxy configuration.
///
/// Combines sudo elevation with proxy settings propagation.
pub fn run_maybe_sudo_with_network(
    use_sudo: bool,
    cmd: &str,
    args: &[&str],
    network: Option<&NetworkConfig>,
    tool_name: &str,
) -> Result<Output, InstallError> {
    match current_os() {
        Os::Windows => run_with_network(cmd, args, network, tool_name),
        Os::Linux | Os::Macos | Os::Bsd => {
            if use_sudo {
                // sudo -E preserves environment (including proxy vars)
                // sudo <cmd> <args...>
                let mut all = Vec::with_capacity(2 + args.len());
                all.push("-E"); // Preserve environment
                all.push(cmd);
                all.extend_from_slice(args);
                run_with_network("sudo", &all, network, tool_name)
            } else {
                run_with_network(cmd, args, network, tool_name)
            }
        }
    }
}

pub fn has(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// Require a single command to exist on PATH, otherwise return a Prereq error with remediation.
pub fn require(cmd: &str, remediation: &'static str) -> Result<(), InstallError> {
    if has(cmd) {
        Ok(())
    } else {
        Err(InstallError::Prereq(remediation))
    }
}

// Require one of multiple candidates (e.g., apt or apt-get)
pub fn require_any<'a>(
    candidates: &[&'a str],
    remediation: &'static str,
) -> Result<&'a str, InstallError> {
    for c in candidates {
        if has(c) {
            return Ok(*c);
        }
    }
    Err(InstallError::Prereq(remediation))
}

/// Check if a command's version satisfies the given requirement.
///
/// Uses proper semantic versioning comparison instead of substring matching.
/// Supports requirements like:
/// - `"latest"` or `"*"`: Always passes
/// - `"3.10"`: Matches 3.10.x
/// - `"3.10.0"`: Exact match
/// - `">= 3.10"`: Minimum version
/// - `">= 3.10, < 4.0"`: Range expression
pub fn cmd_satisfies(cmd: &str, requirement: &str) -> bool {
    if let Ok(out) = Command::new(cmd).arg("--version").output() {
        let version_output = String::from_utf8_lossy(&out.stdout);
        return super::version::version_satisfies(&version_output, requirement);
    }
    false
}

pub fn plan_sudo_attempts(use_sudo: Option<bool>, sudo_available: bool) -> Vec<bool> {
    match use_sudo {
        Some(flag) => vec![flag],
        None => {
            if sudo_available {
                vec![false, true]
            } else {
                vec![false]
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PackageManager {
    Apt,
    Dnf,
    Yum,
    Zypper,
    Pacman,
    Apk,
    Brew,
    BrewCask, // Homebrew casks (GUI apps)
    Winget,
    Choco,
    Pkg, // FreeBSD pkg
}

#[cfg(target_os = "linux")]
pub fn detect_linux_pm() -> Option<PackageManager> {
    use std::{fs, process::Command};
    let has = |c| {
        Command::new(c)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };

    // (Optional) use /etc/os-release to bias choices when you need vendor repos
    // ID / ID_LIKE fields are the standard signals.  [oai_citation:0‡Freedesktop](https://www.freedesktop.org/software/systemd/man/os-release.html?utm_source=chatgpt.com) [oai_citation:1‡Debian Manpages](https://manpages.debian.org/trixie/systemd/os-release.5.en.html?utm_source=chatgpt.com)
    let _os_release = fs::read_to_string("/etc/os-release").unwrap_or_default();

    if has("apt-get") || has("apt") {
        return Some(PackageManager::Apt);
    }
    if has("dnf") {
        return Some(PackageManager::Dnf);
    }
    if has("yum") {
        return Some(PackageManager::Yum);
    }
    if has("zypper") {
        return Some(PackageManager::Zypper);
    }
    if has("pacman") {
        return Some(PackageManager::Pacman);
    }
    if has("apk") {
        return Some(PackageManager::Apk);
    }
    None
}

#[cfg(target_os = "freebsd")]
pub fn detect_bsd_pm() -> Option<PackageManager> {
    use std::process::Command;
    let has = |c| {
        Command::new(c)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };

    if has("pkg") {
        return Some(PackageManager::Pkg);
    }
    None
}

#[cfg(test)]
mod sudo_plan_tests {
    use super::plan_sudo_attempts;

    #[test]
    fn plan_some_true_only_true() {
        let v = plan_sudo_attempts(Some(true), true);
        assert_eq!(v, vec![true]);
    }

    #[test]
    fn plan_some_false_only_false() {
        let v = plan_sudo_attempts(Some(false), true);
        assert_eq!(v, vec![false]);
    }

    #[test]
    fn plan_none_with_sudo_available() {
        let v = plan_sudo_attempts(None, true);
        assert_eq!(v, vec![false, true]);
    }

    #[test]
    fn plan_none_without_sudo_available() {
        let v = plan_sudo_attempts(None, false);
        assert_eq!(v, vec![false]);
    }
}

#[cfg(test)]
mod batch_install_tests {
    use super::*;

    #[test]
    fn empty_list_returns_empty_result() {
        let result = PkgOps::batch_install(PackageManager::Brew, &[], None);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.succeeded.is_empty());
        assert!(result.failed.is_empty());
    }

    #[test]
    fn batch_install_result_default() {
        let result = BatchInstallResult {
            succeeded: vec!["foo".to_string()],
            failed: vec![("bar".to_string(), "error".to_string())],
        };
        assert_eq!(result.succeeded.len(), 1);
        assert_eq!(result.failed.len(), 1);
        assert_eq!(result.succeeded[0], "foo");
        assert_eq!(result.failed[0].0, "bar");
    }

    #[test]
    fn package_manager_has_required_traits() {
        // Test that PackageManager can be used as HashMap key
        let mut map = std::collections::HashMap::new();
        map.insert(PackageManager::Brew, vec!["jq", "ripgrep"]);
        map.insert(PackageManager::Apt, vec!["git", "curl"]);
        assert_eq!(map.len(), 2);
        assert!(map.contains_key(&PackageManager::Brew));
        assert!(map.contains_key(&PackageManager::Apt));
    }

    #[test]
    fn package_manager_equality() {
        assert_eq!(PackageManager::Brew, PackageManager::Brew);
        assert_ne!(PackageManager::Brew, PackageManager::Apt);
        assert_eq!(PackageManager::BrewCask, PackageManager::BrewCask);
        assert_ne!(PackageManager::Winget, PackageManager::Choco);
    }
}

#[allow(dead_code)]
pub struct PkgOps {
    name: &'static str,
}

/// Result of a batch installation operation.
#[derive(Debug)]
pub struct BatchInstallResult {
    /// Packages that were successfully installed
    pub succeeded: Vec<String>,
    /// Packages that failed to install (package name, error message)
    pub failed: Vec<(String, String)>,
}

impl PkgOps {
    /// Install multiple packages in a single batch operation.
    ///
    /// This is more efficient than installing packages one-by-one because:
    /// - Single dependency resolution pass
    /// - Single lock acquisition
    /// - Package manager can optimize internally
    ///
    /// Returns a BatchInstallResult indicating which packages succeeded/failed.
    /// On batch failure, individual packages are retried.
    pub fn batch_install(
        pm: PackageManager,
        packages: &[&str],
        use_sudo: Option<bool>,
    ) -> Result<BatchInstallResult, InstallError> {
        if packages.is_empty() {
            return Ok(BatchInstallResult {
                succeeded: vec![],
                failed: vec![],
            });
        }

        // Single package: use regular install
        if packages.len() == 1 {
            match Self::install(pm, packages[0], use_sudo) {
                Ok(()) => {
                    return Ok(BatchInstallResult {
                        succeeded: vec![packages[0].to_string()],
                        failed: vec![],
                    });
                }
                Err(e) => {
                    return Ok(BatchInstallResult {
                        succeeded: vec![],
                        failed: vec![(packages[0].to_string(), format!("{}", e))],
                    });
                }
            }
        }

        // Try batch install first
        let batch_result = Self::try_batch_install(pm, packages, use_sudo);

        match batch_result {
            Ok(()) => {
                // All packages installed successfully
                Ok(BatchInstallResult {
                    succeeded: packages.iter().map(|s| s.to_string()).collect(),
                    failed: vec![],
                })
            }
            Err(_) => {
                // Batch failed, retry individually to find which packages failed
                let mut succeeded = Vec::new();
                let mut failed = Vec::new();

                for pkg in packages {
                    match Self::install(pm, pkg, use_sudo) {
                        Ok(()) => succeeded.push(pkg.to_string()),
                        Err(e) => failed.push((pkg.to_string(), format!("{}", e))),
                    }
                }

                Ok(BatchInstallResult { succeeded, failed })
            }
        }
    }

    /// Attempt to install multiple packages in a single command.
    /// Returns Ok if all packages installed, Err if the command failed.
    fn try_batch_install(
        pm: PackageManager,
        packages: &[&str],
        use_sudo: Option<bool>,
    ) -> Result<(), InstallError> {
        match pm {
            PackageManager::Apt => {
                let apt = require_any(&["apt-get", "apt"], "apt is required to install packages")?;
                let mut args = vec!["install", "-y"];
                args.extend(packages);
                Self::run_with_sudo_strategy(use_sudo, apt, &args)
            }
            PackageManager::Dnf => {
                require("dnf", "dnf is required to install packages")?;
                let mut args = vec!["install", "-y"];
                args.extend(packages);
                Self::run_with_sudo_strategy(use_sudo, "dnf", &args)
            }
            PackageManager::Yum => {
                require("yum", "yum is required to install packages")?;
                let mut args = vec!["install", "-y"];
                args.extend(packages);
                Self::run_with_sudo_strategy(use_sudo, "yum", &args)
            }
            PackageManager::Zypper => {
                require("zypper", "zypper is required to install packages")?;
                let mut args = vec!["--non-interactive", "install", "--no-confirm"];
                args.extend(packages);
                Self::run_with_sudo_strategy(use_sudo, "zypper", &args)
            }
            PackageManager::Pacman => {
                require("pacman", "pacman is required to install packages")?;
                let mut args = vec!["--noconfirm", "-S"];
                args.extend(packages);
                Self::run_with_sudo_strategy(use_sudo, "pacman", &args)
            }
            PackageManager::Apk => {
                require("apk", "apk is required to install packages")?;
                let mut args = vec!["add"];
                args.extend(packages);
                Self::run_with_sudo_strategy(use_sudo, "apk", &args)
            }
            PackageManager::Brew => {
                require("brew", "Homebrew is required to install packages")?;
                let mut args = vec!["install"];
                args.extend(packages);
                run("brew", &args)?;
                Ok(())
            }
            PackageManager::Winget => {
                // winget doesn't support true batch install, but we can chain commands
                // For now, install sequentially since winget is internally sequential anyway
                require("winget", "Winget is required to install packages")?;
                for pkg in packages {
                    run("winget", &["install", "-e", "--id", pkg])?;
                }
                Ok(())
            }
            PackageManager::BrewCask => {
                require("brew", "Homebrew is required to install casks")?;
                let mut args = vec!["install", "--cask"];
                args.extend(packages);
                run("brew", &args)?;
                Ok(())
            }
            PackageManager::Choco => {
                require("choco", "Chocolatey is required to install packages")?;
                let mut args = vec!["install", "-y"];
                args.extend(packages);
                run("choco", &args)?;
                Ok(())
            }
            PackageManager::Pkg => {
                require("pkg", "FreeBSD pkg is required to install packages")?;
                let mut args = vec!["install", "-y"];
                args.extend(packages);
                Self::run_with_sudo_strategy(use_sudo, "pkg", &args)
            }
        }
    }

    /// Helper to run a command with sudo fallback strategy.
    fn run_with_sudo_strategy(
        use_sudo: Option<bool>,
        cmd: &str,
        args: &[&str],
    ) -> Result<(), InstallError> {
        match use_sudo {
            Some(flag) => {
                if flag {
                    require("sudo", "sudo is required to install packages")?;
                }
                run_maybe_sudo(flag, cmd, args)?;
                Ok(())
            }
            None => {
                if let Err(e) = run_maybe_sudo(false, cmd, args) {
                    if has("sudo") {
                        run_maybe_sudo(true, cmd, args)?;
                        Ok(())
                    } else {
                        Err(e)
                    }
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Install packages using Homebrew cask (for GUI apps) in batch.
    pub fn batch_install_cask(packages: &[&str]) -> Result<BatchInstallResult, InstallError> {
        if packages.is_empty() {
            return Ok(BatchInstallResult {
                succeeded: vec![],
                failed: vec![],
            });
        }

        require("brew", "Homebrew is required to install casks")?;

        let mut args = vec!["install", "--cask"];
        args.extend(packages);

        match run("brew", &args) {
            Ok(_) => Ok(BatchInstallResult {
                succeeded: packages.iter().map(|s| s.to_string()).collect(),
                failed: vec![],
            }),
            Err(_) => {
                // Retry individually
                let mut succeeded = Vec::new();
                let mut failed = Vec::new();
                for pkg in packages {
                    match run("brew", &["install", "--cask", pkg]) {
                        Ok(_) => succeeded.push(pkg.to_string()),
                        Err(e) => failed.push((pkg.to_string(), format!("{}", e))),
                    }
                }
                Ok(BatchInstallResult { succeeded, failed })
            }
        }
    }

    /// Install packages using Chocolatey in batch.
    pub fn batch_install_choco(packages: &[&str]) -> Result<BatchInstallResult, InstallError> {
        if packages.is_empty() {
            return Ok(BatchInstallResult {
                succeeded: vec![],
                failed: vec![],
            });
        }

        require("choco", "Chocolatey is required to install packages")?;

        let mut args = vec!["install", "-y"];
        args.extend(packages);

        match run("choco", &args) {
            Ok(_) => Ok(BatchInstallResult {
                succeeded: packages.iter().map(|s| s.to_string()).collect(),
                failed: vec![],
            }),
            Err(_) => {
                // Retry individually
                let mut succeeded = Vec::new();
                let mut failed = Vec::new();
                for pkg in packages {
                    match run("choco", &["install", "-y", pkg]) {
                        Ok(_) => succeeded.push(pkg.to_string()),
                        Err(e) => failed.push((pkg.to_string(), format!("{}", e))),
                    }
                }
                Ok(BatchInstallResult { succeeded, failed })
            }
        }
    }

    pub fn update(pm: PackageManager, use_sudo: Option<bool>) -> Result<(), InstallError> {
        match pm {
            PackageManager::Apt => {
                // Ensure prerequisites exist before attempting the update
                let apt = require_any(&["apt-get", "apt"], "apt is required to update packages")?;
                // Decide sudo strategy
                match use_sudo {
                    Some(flag) => {
                        if flag {
                            require("sudo", "sudo is required to update packages")?;
                        }
                        run_maybe_sudo(flag, apt, &["update"])?;
                    }
                    None => {
                        // Try without sudo first
                        if let Err(e) = run_maybe_sudo(false, apt, &["update"]) {
                            // Retry with sudo if available
                            if has("sudo") {
                                run_maybe_sudo(true, apt, &["update"])?;
                            } else {
                                return Err(e);
                            }
                        }
                    }
                }
            }
            PackageManager::Dnf => { /* dnf auto-refreshes; optional */ }
            PackageManager::Yum => { /* optional */ }
            PackageManager::Zypper => {
                require("zypper", "zypper is required to update packages")?;
                match use_sudo {
                    Some(flag) => {
                        if flag {
                            require("sudo", "sudo is required to update packages")?;
                        }
                        run_maybe_sudo(flag, "zypper", &["--non-interactive", "refresh"])?;
                    }
                    None => {
                        if let Err(e) =
                            run_maybe_sudo(false, "zypper", &["--non-interactive", "refresh"])
                        {
                            if has("sudo") {
                                run_maybe_sudo(true, "zypper", &["--non-interactive", "refresh"])?;
                            } else {
                                return Err(e);
                            }
                        }
                    }
                }
            }
            PackageManager::Pacman => {
                require("pacman", "pacman is required to update packages")?;
                match use_sudo {
                    Some(flag) => {
                        if flag {
                            require("sudo", "sudo is required to update packages")?;
                        }
                        run_maybe_sudo(flag, "pacman", &["-Sy"])?;
                    }
                    None => {
                        if let Err(e) = run_maybe_sudo(false, "pacman", &["-Sy"]) {
                            if has("sudo") {
                                run_maybe_sudo(true, "pacman", &["-Sy"])?;
                            } else {
                                return Err(e);
                            }
                        }
                    }
                }
            }
            PackageManager::Apk => { /* `apk add` refreshes on demand */ }
            PackageManager::Pkg => {
                require("pkg", "FreeBSD pkg is required to update packages")?;
                match use_sudo {
                    Some(flag) => {
                        if flag {
                            require("sudo", "sudo is required to update packages")?;
                        }
                        run_maybe_sudo(flag, "pkg", &["update"])?;
                    }
                    None => {
                        if let Err(e) = run_maybe_sudo(false, "pkg", &["update"]) {
                            if has("sudo") {
                                run_maybe_sudo(true, "pkg", &["update"])?;
                            } else {
                                return Err(e);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn install(
        pm: PackageManager,
        pkg: &str,
        use_sudo: Option<bool>,
    ) -> Result<(), InstallError> {
        match pm {
            PackageManager::Apt => {
                let apt = require_any(&["apt-get", "apt"], "apt is required to install packages")?;
                match use_sudo {
                    Some(flag) => {
                        if flag {
                            require("sudo", "sudo is required to install packages")?;
                        }
                        run_maybe_sudo(flag, apt, &["install", "-y", pkg])?;
                    }
                    None => {
                        if let Err(e) = run_maybe_sudo(false, apt, &["install", "-y", pkg]) {
                            if has("sudo") {
                                run_maybe_sudo(true, apt, &["install", "-y", pkg])?;
                            } else {
                                return Err(e);
                            }
                        }
                    }
                }
            }
            PackageManager::Dnf => {
                require("dnf", "dnf is required to install packages")?;
                match use_sudo {
                    Some(flag) => {
                        if flag {
                            require("sudo", "sudo is required to install packages")?;
                        }
                        run_maybe_sudo(flag, "dnf", &["install", "-y", pkg])?;
                    }
                    None => {
                        if let Err(e) = run_maybe_sudo(false, "dnf", &["install", "-y", pkg]) {
                            if has("sudo") {
                                run_maybe_sudo(true, "dnf", &["install", "-y", pkg])?;
                            } else {
                                return Err(e);
                            }
                        }
                    }
                }
            }
            PackageManager::Yum => {
                require("yum", "yum is required to install packages")?;
                match use_sudo {
                    Some(flag) => {
                        if flag {
                            require("sudo", "sudo is required to install packages")?;
                        }
                        run_maybe_sudo(flag, "yum", &["install", "-y", pkg])?;
                    }
                    None => {
                        if let Err(e) = run_maybe_sudo(false, "yum", &["install", "-y", pkg]) {
                            if has("sudo") {
                                run_maybe_sudo(true, "yum", &["install", "-y", pkg])?;
                            } else {
                                return Err(e);
                            }
                        }
                    }
                }
            }
            PackageManager::Zypper => {
                require("zypper", "zypper is required to install packages")?;
                match use_sudo {
                    Some(flag) => {
                        if flag {
                            require("sudo", "sudo is required to install packages")?;
                        }
                        run_maybe_sudo(
                            flag,
                            "zypper",
                            &["--non-interactive", "install", "--no-confirm", pkg],
                        )?;
                    }
                    None => {
                        if let Err(e) = run_maybe_sudo(
                            false,
                            "zypper",
                            &["--non-interactive", "install", "--no-confirm", pkg],
                        ) {
                            if has("sudo") {
                                run_maybe_sudo(
                                    true,
                                    "zypper",
                                    &["--non-interactive", "install", "--no-confirm", pkg],
                                )?;
                            } else {
                                return Err(e);
                            }
                        }
                    }
                }
            }
            PackageManager::Pacman => {
                require("pacman", "pacman is required to install packages")?;
                match use_sudo {
                    Some(flag) => {
                        if flag {
                            require("sudo", "sudo is required to install packages")?;
                        }
                        run_maybe_sudo(flag, "pacman", &["--noconfirm", "-S", pkg])?;
                    }
                    None => {
                        if let Err(e) = run_maybe_sudo(false, "pacman", &["--noconfirm", "-S", pkg])
                        {
                            if has("sudo") {
                                run_maybe_sudo(true, "pacman", &["--noconfirm", "-S", pkg])?;
                            } else {
                                return Err(e);
                            }
                        }
                    }
                }
            }
            PackageManager::Apk => {
                require("apk", "apk is required to install packages")?;
                match use_sudo {
                    Some(flag) => {
                        if flag {
                            require("sudo", "sudo is required to install packages")?;
                        }
                        run_maybe_sudo(flag, "apk", &["add", pkg])?;
                    }
                    None => {
                        if let Err(e) = run_maybe_sudo(false, "apk", &["add", pkg]) {
                            if has("sudo") {
                                run_maybe_sudo(true, "apk", &["add", pkg])?;
                            } else {
                                return Err(e);
                            }
                        }
                    }
                }
            }
            // These package managers generally do not require sudo by design here
            PackageManager::Brew => {
                require("brew", "Homebrew is required to install packages")?;
                run("brew", &["install", pkg])?;
            }
            PackageManager::BrewCask => {
                require("brew", "Homebrew is required to install casks")?;
                run("brew", &["install", "--cask", pkg])?;
            }
            PackageManager::Winget => {
                require("winget", "Winget is required to install packages")?;
                run("winget", &["install", "-e", "--id", pkg])?;
            }
            PackageManager::Choco => {
                require("choco", "Chocolatey is required to install packages")?;
                run("choco", &["install", "-y", pkg])?;
            }
            PackageManager::Pkg => {
                require("pkg", "FreeBSD pkg is required to install packages")?;
                match use_sudo {
                    Some(flag) => {
                        if flag {
                            require("sudo", "sudo is required to install packages")?;
                        }
                        run_maybe_sudo(flag, "pkg", &["install", "-y", pkg])?;
                    }
                    None => {
                        if let Err(e) = run_maybe_sudo(false, "pkg", &["install", "-y", pkg]) {
                            if has("sudo") {
                                run_maybe_sudo(true, "pkg", &["install", "-y", pkg])?;
                            } else {
                                return Err(e);
                            }
                        }
                    }
                }
            }
        };
        Ok(())
    }
}
