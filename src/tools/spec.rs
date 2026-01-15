//! Declarative tool specification pattern for reducing boilerplate.
//!
//! This module provides the `ToolSpec` struct and `define_tool!` macro that
//! eliminates ~80% of code duplication across tool implementations.
//!
//! Tools defined with `define_tool!` are automatically registered via the
//! `inventory` crate, eliminating the need for manual registration in
//! `register_all()`.
//!
//! # Example
//!
//! ```ignore
//! use jarvy::tools::spec::ToolSpec;
//!
//! pub static GIT: ToolSpec = ToolSpec {
//!     name: "git",
//!     command: "git",
//!     macos: Some(MacOsInstall { brew: Some("git"), cask: None }),
//!     linux: Some(LinuxInstall::uniform("git")),
//!     windows: Some(WindowsInstall { winget: Some("Git.Git"), choco: Some("git") }),
//!     custom_install: None,
//! };
//! ```

use super::common::{cmd_satisfies, has, run, InstallError, PackageManager};

/// Type alias for tool handler functions registered in the registry.
pub type ToolHandler = fn(&str) -> Result<(), InstallError>;

/// A wrapper for tool registration data to enable inventory collection.
/// This allows tools defined with `define_tool!` to be automatically discovered
/// and registered at runtime without manual registration.
///
/// Contains:
/// - The tool's static specification (`&'static ToolSpec`)
/// - The handler function pointer for registry registration
pub struct ToolEntry {
    pub spec: &'static ToolSpec,
    pub handler: ToolHandler,
}

// Enable inventory collection for ToolEntry
inventory::collect!(ToolEntry);

/// Iterate over all registered tool entries (spec + handler).
/// Used by `register_all()` to automatically register all ToolSpec-based tools.
pub fn iter_tools() -> impl Iterator<Item = &'static ToolEntry> {
    inventory::iter::<ToolEntry>.into_iter()
}

#[cfg(target_os = "linux")]
use super::common::{default_use_sudo, PkgOps};

/// macOS installation options.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct MacOsInstall {
    /// Homebrew formula name (e.g., "git", "jq")
    pub brew: Option<&'static str>,
    /// Homebrew cask name (e.g., "visual-studio-code", "docker")
    pub cask: Option<&'static str>,
}

impl MacOsInstall {
    /// Create a MacOsInstall with just a Homebrew formula.
    pub const fn brew(name: &'static str) -> Self {
        Self {
            brew: Some(name),
            cask: None,
        }
    }

    /// Create a MacOsInstall with just a Homebrew cask.
    pub const fn cask(name: &'static str) -> Self {
        Self {
            brew: None,
            cask: Some(name),
        }
    }
}

/// Linux installation options for various package managers.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct LinuxInstall {
    pub apt: Option<&'static str>,
    pub dnf: Option<&'static str>,
    pub yum: Option<&'static str>,
    pub zypper: Option<&'static str>,
    pub pacman: Option<&'static str>,
    pub apk: Option<&'static str>,
    /// Homebrew formula for Linuxbrew (used when native packages unavailable)
    pub brew: Option<&'static str>,
}

impl LinuxInstall {
    /// Create a LinuxInstall where all package managers use the same package name.
    pub const fn uniform(name: &'static str) -> Self {
        Self {
            apt: Some(name),
            dnf: Some(name),
            yum: Some(name),
            zypper: Some(name),
            pacman: Some(name),
            apk: Some(name),
            brew: None,
        }
    }

    /// Create a LinuxInstall that uses Linuxbrew (for tools without native packages).
    pub const fn brew(name: &'static str) -> Self {
        Self {
            apt: None,
            dnf: None,
            yum: None,
            zypper: None,
            pacman: None,
            apk: None,
            brew: Some(name),
        }
    }

    /// Create an empty LinuxInstall (no packages defined).
    pub const fn none() -> Self {
        Self {
            apt: None,
            dnf: None,
            yum: None,
            zypper: None,
            pacman: None,
            apk: None,
            brew: None,
        }
    }

    /// Get the package name for a specific package manager.
    pub fn get(&self, pm: PackageManager) -> Option<&'static str> {
        match pm {
            PackageManager::Apt => self.apt,
            PackageManager::Dnf => self.dnf,
            PackageManager::Yum => self.yum,
            PackageManager::Zypper => self.zypper,
            PackageManager::Pacman => self.pacman,
            PackageManager::Apk => self.apk,
            PackageManager::Brew => self.brew,
            _ => None,
        }
    }
}

/// Windows installation options.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct WindowsInstall {
    /// winget package ID (e.g., "Git.Git", "jqlang.jq")
    pub winget: Option<&'static str>,
    /// Chocolatey package name
    pub choco: Option<&'static str>,
}

impl WindowsInstall {
    /// Create a WindowsInstall with just a winget ID.
    pub const fn winget(id: &'static str) -> Self {
        Self {
            winget: Some(id),
            choco: None,
        }
    }

    /// Create a WindowsInstall with just a Chocolatey package.
    pub const fn choco(name: &'static str) -> Self {
        Self {
            winget: None,
            choco: Some(name),
        }
    }
}

/// Type alias for custom installation functions.
pub type CustomInstallFn = fn(&str) -> Result<(), InstallError>;

/// Default post-install hook configuration for a tool.
///
/// Tools can define a default hook that runs after installation to configure
/// the tool (e.g., adding shell integration, setting up PATH, creating configs).
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct DefaultHook {
    /// Human-readable description of what the hook does.
    /// Displayed to users in dry-run mode and hook listings.
    pub description: &'static str,

    /// The shell script to execute.
    /// This should be idempotent (safe to run multiple times).
    pub script: &'static str,

    /// Optional platform filter: "macos", "linux", "windows", or None for all platforms.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<&'static str>,
}

impl DefaultHook {
    /// Create a new default hook for all platforms.
    pub const fn new(description: &'static str, script: &'static str) -> Self {
        Self {
            description,
            script,
            platform: None,
        }
    }

    /// Create a new default hook for a specific platform.
    pub const fn for_platform(
        description: &'static str,
        script: &'static str,
        platform: &'static str,
    ) -> Self {
        Self {
            description,
            script,
            platform: Some(platform),
        }
    }

    /// Check if this hook should run on the current platform.
    pub fn should_run_on_current_platform(&self) -> bool {
        match self.platform {
            None => true,
            Some("macos") => cfg!(target_os = "macos"),
            Some("linux") => cfg!(target_os = "linux"),
            Some("windows") => cfg!(target_os = "windows"),
            Some(_) => false,
        }
    }
}

/// Declarative tool specification that eliminates boilerplate.
///
/// A `ToolSpec` defines everything needed to check for and install a tool
/// across all supported platforms. The `ensure()` method handles the common
/// pattern of checking version satisfaction and installing if needed.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct ToolSpec {
    /// Tool name for registry and display (e.g., "git", "docker")
    pub name: &'static str,

    /// Command to check existence (usually same as name, e.g., "git")
    pub command: &'static str,

    /// macOS installation options (None if not supported on macOS)
    pub macos: Option<MacOsInstall>,

    /// Linux installation options (None if not supported on Linux)
    pub linux: Option<LinuxInstall>,

    /// Windows installation options (None if not supported on Windows)
    pub windows: Option<WindowsInstall>,

    /// Optional custom install function for complex tools (nvm, rustup, etc.)
    /// If provided, this takes precedence over standard package manager installs.
    #[serde(skip)]
    pub custom_install: Option<CustomInstallFn>,

    /// Optional default post-install hook that runs after tool installation.
    /// Used for shell integration, PATH setup, config generation, etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_hook: Option<DefaultHook>,
}

impl ToolSpec {
    /// Check if the tool is installed and satisfies the version requirement.
    pub fn is_satisfied(&self, min_hint: &str) -> bool {
        cmd_satisfies(self.command, min_hint)
    }

    /// Get the default hook if one exists and should run on the current platform.
    pub fn get_default_hook(&self) -> Option<&DefaultHook> {
        self.default_hook
            .as_ref()
            .filter(|h| h.should_run_on_current_platform())
    }

    /// Check if this tool has a default hook that would run on the current platform.
    pub fn has_default_hook(&self) -> bool {
        self.get_default_hook().is_some()
    }

    /// Ensure the tool is installed and satisfies the version requirement.
    ///
    /// This is the main entry point that replaces the boilerplate `ensure()` function
    /// in each tool file. It checks if the tool satisfies the requirement and installs
    /// if needed.
    pub fn ensure(&self, min_hint: &str) -> Result<(), InstallError> {
        if self.is_satisfied(min_hint) {
            return Ok(());
        }
        self.install(min_hint)
    }

    /// Install the tool using the appropriate method for the current platform.
    fn install(&self, min_hint: &str) -> Result<(), InstallError> {
        // Check for custom installer first
        if let Some(custom_fn) = self.custom_install {
            return custom_fn(min_hint);
        }

        // Platform-specific installation
        #[cfg(target_os = "macos")]
        {
            return self.install_macos();
        }
        #[cfg(target_os = "linux")]
        {
            return self.install_linux();
        }
        #[cfg(target_os = "windows")]
        {
            return self.install_windows();
        }
        #[allow(unreachable_code)]
        Err(InstallError::Unsupported)
    }

    #[cfg(target_os = "macos")]
    fn install_macos(&self) -> Result<(), InstallError> {
        let macos = self.macos.ok_or(InstallError::Unsupported)?;

        // Try cask first if specified (typically for GUI apps)
        if let Some(cask_name) = macos.cask {
            if !has("brew") {
                return Err(InstallError::Prereq(
                    "Homebrew not found. Install https://brew.sh and re-run.",
                ));
            }
            run("brew", &["install", "--cask", cask_name])?;
            return Ok(());
        }

        // Otherwise use formula
        if let Some(formula) = macos.brew {
            if !has("brew") {
                return Err(InstallError::Prereq(
                    "Homebrew not found. Install https://brew.sh and re-run.",
                ));
            }
            run("brew", &["install", formula])?;
            return Ok(());
        }

        Err(InstallError::Unsupported)
    }

    #[cfg(target_os = "linux")]
    fn install_linux(&self) -> Result<(), InstallError> {
        let linux = self.linux.ok_or(InstallError::Unsupported)?;

        // Try native package manager first
        if let Some(pm) = super::common::detect_linux_pm() {
            if let Some(pkg_name) = linux.get(pm) {
                let _ = PkgOps::update(pm, default_use_sudo());
                return PkgOps::install(pm, pkg_name, default_use_sudo());
            }
        }

        // Fallback to Linuxbrew if available
        if let Some(brew_pkg) = linux.brew {
            if has("brew") {
                run("brew", &["install", brew_pkg])?;
                return Ok(());
            }
        }

        Err(InstallError::Prereq(
            "No supported Linux package manager on PATH (apt/dnf/yum/zypper/pacman/apk/brew)",
        ))
    }

    #[cfg(target_os = "windows")]
    fn install_windows(&self) -> Result<(), InstallError> {
        let windows = self.windows.ok_or(InstallError::Unsupported)?;

        // Prefer winget
        if let Some(winget_id) = windows.winget {
            if has("winget") {
                run("winget", &["install", "-e", "--id", winget_id])?;
                return Ok(());
            }
        }

        // Fallback to chocolatey
        if let Some(choco_pkg) = windows.choco {
            if has("choco") {
                run("choco", &["install", "-y", choco_pkg])?;
                return Ok(());
            }
        }

        // Neither package manager available
        if windows.winget.is_some() {
            Err(InstallError::Prereq(
                "winget not found. Install Windows Package Manager, then re-run.",
            ))
        } else {
            Err(InstallError::Prereq(
                "chocolatey not found. Install Chocolatey, then re-run.",
            ))
        }
    }

}

/// Macro for defining tools with minimal boilerplate.
///
/// Tools defined with this macro are automatically registered via the `inventory`
/// crate, eliminating the need for manual registration in `register_all()`.
///
/// # Example
///
/// ```ignore
/// define_tool!(git, {
///     command: "git",
///     macos: { brew: "git" },
///     linux: { uniform: "git" },
///     windows: { winget: "Git.Git" },
/// });
/// ```
#[macro_export]
macro_rules! define_tool {
    // Full form with all platforms
    ($name:ident, {
        command: $cmd:expr,
        $(macos: { $($macos_key:ident: $macos_val:expr),* $(,)? },)?
        $(linux: { $($linux_key:ident: $linux_val:expr),* $(,)? },)?
        $(windows: { $($windows_key:ident: $windows_val:expr),* $(,)? },)?
        $(custom_install: $custom:expr,)?
        $(default_hook: { description: $hook_desc:expr, script: $hook_script:expr $(, platform: $hook_platform:expr)? },)?
    }) => {
        pub static $name: $crate::tools::spec::ToolSpec = $crate::tools::spec::ToolSpec {
            name: stringify!($name),
            command: $cmd,
            macos: define_tool!(@macos $($($macos_key: $macos_val),*)?),
            linux: define_tool!(@linux $($($linux_key: $linux_val),*)?),
            windows: define_tool!(@windows $($($windows_key: $windows_val),*)?),
            custom_install: define_tool!(@custom $($custom)?),
            default_hook: define_tool!(@default_hook $($hook_desc, $hook_script $(, $hook_platform)?)?),
        };

        pub fn ensure(min_hint: &str) -> Result<(), $crate::tools::common::InstallError> {
            $name.ensure(min_hint)
        }

        pub fn add_handler(min_hint: &str) -> Result<(), $crate::tools::common::InstallError> {
            $name.ensure(min_hint)
        }

        // Auto-register this tool with inventory (must be after handler definition)
        ::inventory::submit! {
            $crate::tools::spec::ToolEntry {
                spec: &$name,
                handler: add_handler,
            }
        }
    };

    // macOS helpers
    (@macos) => { None };
    (@macos brew: $val:expr) => {
        Some($crate::tools::spec::MacOsInstall::brew($val))
    };
    (@macos cask: $val:expr) => {
        Some($crate::tools::spec::MacOsInstall::cask($val))
    };
    (@macos brew: $brew:expr, cask: $cask:expr) => {
        Some($crate::tools::spec::MacOsInstall { brew: Some($brew), cask: Some($cask) })
    };

    // Linux helpers
    (@linux) => { None };
    (@linux uniform: $val:expr) => {
        Some($crate::tools::spec::LinuxInstall::uniform($val))
    };
    // Linuxbrew support (for tools like upbound/tap/up)
    (@linux brew: $val:expr) => {
        Some($crate::tools::spec::LinuxInstall::brew($val))
    };
    (@linux apt: $apt:expr, dnf: $dnf:expr, pacman: $pacman:expr, apk: $apk:expr) => {
        Some($crate::tools::spec::LinuxInstall {
            apt: Some($apt),
            dnf: Some($dnf),
            yum: Some($dnf), // often same as dnf
            zypper: Some($dnf),
            pacman: Some($pacman),
            apk: Some($apk),
            brew: None,
        })
    };
    (@linux apt: $apt:expr, dnf: $dnf:expr, yum: $yum:expr, zypper: $zypper:expr, pacman: $pacman:expr, apk: $apk:expr) => {
        Some($crate::tools::spec::LinuxInstall {
            apt: Some($apt),
            dnf: Some($dnf),
            yum: Some($yum),
            zypper: Some($zypper),
            pacman: Some($pacman),
            apk: Some($apk),
            brew: None,
        })
    };

    // Windows helpers
    (@windows) => { None };
    (@windows winget: $val:expr) => {
        Some($crate::tools::spec::WindowsInstall::winget($val))
    };
    (@windows choco: $val:expr) => {
        Some($crate::tools::spec::WindowsInstall::choco($val))
    };
    (@windows winget: $winget:expr, choco: $choco:expr) => {
        Some($crate::tools::spec::WindowsInstall { winget: Some($winget), choco: Some($choco) })
    };

    // Custom install helper
    (@custom) => { None };
    (@custom $fn:expr) => { Some($fn) };

    // Default hook helpers
    (@default_hook) => { None };
    (@default_hook $desc:expr, $script:expr) => {
        Some($crate::tools::spec::DefaultHook::new($desc, $script))
    };
    (@default_hook $desc:expr, $script:expr, $platform:expr) => {
        Some($crate::tools::spec::DefaultHook::for_platform($desc, $script, $platform))
    };
}

pub use define_tool;

// ============================================================================
// Tool Index Generation
// ============================================================================

/// Metadata about a tool's custom installation support.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CustomInstallInfo {
    /// Whether this tool uses a custom installer (shell script, etc.)
    pub has_custom_installer: bool,
}

/// A serializable tool entry for the tool index.
/// This includes the full ToolSpec data plus metadata about custom installers.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolIndexEntry {
    /// Tool name
    pub name: String,
    /// Command used to check if installed
    pub command: String,
    /// macOS installation options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub macos: Option<MacOsInstall>,
    /// Linux installation options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linux: Option<LinuxInstall>,
    /// Windows installation options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub windows: Option<WindowsInstall>,
    /// Custom installation info
    pub custom_install: CustomInstallInfo,
}

impl From<&ToolSpec> for ToolIndexEntry {
    fn from(spec: &ToolSpec) -> Self {
        Self {
            // Normalize name to lowercase for consistency
            name: spec.name.to_lowercase(),
            command: spec.command.to_string(),
            macos: spec.macos,
            linux: spec.linux,
            windows: spec.windows,
            custom_install: CustomInstallInfo {
                has_custom_installer: spec.custom_install.is_some(),
            },
        }
    }
}

/// The complete tool index containing all supported tools.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolIndex {
    /// Version of the index format
    pub version: &'static str,
    /// Total count of supported tools
    pub count: usize,
    /// List of all tool entries
    pub tools: Vec<ToolIndexEntry>,
}

impl ToolIndex {
    /// Current version of the tool index format.
    pub const VERSION: &'static str = "1.0.0";
}

/// Manually registered tools that don't use the `define_tool!` macro.
/// These tools have custom installation logic and are registered in `register_all()`.
const MANUAL_TOOLS: &[(&str, &str)] = &[
    ("nvm", "nvm"),
    ("rust", "rustc"),
    ("brew", "brew"),
];

/// Generate the complete tool index by collecting all tools.
///
/// This includes:
/// - All tools registered via `define_tool!` macro (collected by inventory)
/// - Manually registered tools (nvm, rust, brew) with custom installers
pub fn generate_tool_index() -> ToolIndex {
    let mut tools: Vec<ToolIndexEntry> = Vec::new();

    // Collect all tools from inventory (define_tool! macro)
    for entry in iter_tools() {
        tools.push(ToolIndexEntry::from(entry.spec));
    }

    // Add manually registered tools
    for (name, command) in MANUAL_TOOLS {
        tools.push(ToolIndexEntry {
            name: name.to_string(),
            command: command.to_string(),
            macos: None,
            linux: None,
            windows: None,
            custom_install: CustomInstallInfo {
                has_custom_installer: true,
            },
        });
    }

    // Sort by name for consistent output
    tools.sort_by(|a, b| a.name.cmp(&b.name));

    ToolIndex {
        version: ToolIndex::VERSION,
        count: tools.len(),
        tools,
    }
}

/// Generate the tool index as a JSON string.
pub fn generate_tool_index_json() -> String {
    let index = generate_tool_index();
    serde_json::to_string_pretty(&index).unwrap_or_else(|e| {
        format!(r#"{{"error": "{}"}}"#, e)
    })
}

/// Get a list of all supported tool names (lowercase).
pub fn list_tool_names() -> Vec<String> {
    let mut names: Vec<String> = iter_tools()
        .map(|e| e.spec.name.to_lowercase())
        .collect();

    // Add manually registered tools
    for (name, _) in MANUAL_TOOLS {
        names.push(name.to_string());
    }

    names.sort();
    names
}

/// Look up a ToolSpec by name (case-insensitive).
/// Returns None if the tool is not found or is a manually registered tool.
pub fn get_tool_spec(name: &str) -> Option<&'static ToolSpec> {
    let name_lower = name.to_lowercase();
    iter_tools()
        .find(|entry| entry.spec.name.to_lowercase() == name_lower)
        .map(|entry| entry.spec)
}

/// Get the default hook for a tool by name, if one exists for the current platform.
pub fn get_tool_default_hook(name: &str) -> Option<&'static DefaultHook> {
    get_tool_spec(name).and_then(|spec| spec.get_default_hook())
}

/// Get a list of all tools that have default hooks (for the current platform).
pub fn list_tools_with_default_hooks() -> Vec<(&'static str, &'static DefaultHook)> {
    iter_tools()
        .filter_map(|entry| {
            entry
                .spec
                .get_default_hook()
                .map(|hook| (entry.spec.name, hook))
        })
        .collect()
}

// ============================================================================
// Parallel Version Checking
// ============================================================================

use rayon::prelude::*;

/// Result of checking a tool's version status.
#[derive(Debug, Clone)]
pub struct ToolVersionStatus {
    /// Tool name
    pub name: String,
    /// Version requirement from config
    pub version: String,
    /// Whether the tool is already installed with a satisfying version
    pub satisfied: bool,
    /// Whether this tool exists in the registry
    pub known: bool,
}

/// Summary of parallel version check results.
#[derive(Debug, Default)]
pub struct VersionCheckSummary {
    /// Tools that are already satisfied (no installation needed)
    pub satisfied: Vec<(String, String)>,
    /// Tools that need installation
    pub needs_install: Vec<(String, String)>,
    /// Tools not found in registry
    pub unknown: Vec<(String, String)>,
    /// Duration of the check in milliseconds
    pub duration_ms: u64,
}

impl VersionCheckSummary {
    /// Get a human-readable summary string.
    pub fn summary_string(&self) -> String {
        format!(
            "Version check: {} satisfied, {} need install, {} unknown ({}ms)",
            self.satisfied.len(),
            self.needs_install.len(),
            self.unknown.len(),
            self.duration_ms
        )
    }
}

/// Check version status for a single tool.
fn check_tool_version(name: &str, version: &str) -> ToolVersionStatus {
    let name_lower = name.to_lowercase();

    // Check if tool is known
    let spec = get_tool_spec(&name_lower);
    let is_manual = MANUAL_TOOLS.iter().any(|(n, _)| *n == name_lower);

    if spec.is_none() && !is_manual {
        return ToolVersionStatus {
            name: name.to_string(),
            version: version.to_string(),
            satisfied: false,
            known: false,
        };
    }

    // Check if version is satisfied
    let satisfied = if let Some(spec) = spec {
        spec.is_satisfied(version)
    } else {
        // Manual tools (nvm, rust, brew) - check if command exists
        let (_, cmd) = MANUAL_TOOLS.iter().find(|(n, _)| *n == name_lower).unwrap();
        has(*cmd)
    };

    ToolVersionStatus {
        name: name.to_string(),
        version: version.to_string(),
        satisfied,
        known: true,
    }
}

/// Check version status for multiple tools in parallel.
///
/// This uses rayon's parallel iterator to check all tools concurrently,
/// significantly speeding up the version checking phase for large tool lists.
pub fn check_tools_parallel<'a, I>(tools: I) -> VersionCheckSummary
where
    I: Iterator<Item = (&'a str, &'a str)>,
{
    let start = std::time::Instant::now();

    // Collect tools into a Vec for parallel processing
    let tool_list: Vec<(&str, &str)> = tools.collect();

    // Check all tools in parallel
    let results: Vec<ToolVersionStatus> = tool_list
        .par_iter()
        .map(|(name, version)| check_tool_version(name, version))
        .collect();

    // Categorize results
    let mut summary = VersionCheckSummary::default();

    for status in results {
        if !status.known {
            summary.unknown.push((status.name, status.version));
        } else if status.satisfied {
            summary.satisfied.push((status.name, status.version));
        } else {
            summary.needs_install.push((status.name, status.version));
        }
    }

    summary.duration_ms = start.elapsed().as_millis() as u64;
    summary
}

/// Check version status for multiple tools sequentially (for comparison/fallback).
pub fn check_tools_sequential<'a, I>(tools: I) -> VersionCheckSummary
where
    I: Iterator<Item = (&'a str, &'a str)>,
{
    let start = std::time::Instant::now();

    let mut summary = VersionCheckSummary::default();

    for (name, version) in tools {
        let status = check_tool_version(name, version);
        if !status.known {
            summary.unknown.push((status.name, status.version));
        } else if status.satisfied {
            summary.satisfied.push((status.name, status.version));
        } else {
            summary.needs_install.push((status.name, status.version));
        }
    }

    summary.duration_ms = start.elapsed().as_millis() as u64;
    summary
}

// ============================================================================
// Tool Grouping for Batch Installation
// ============================================================================

/// Information about how to install a tool via a package manager.
#[derive(Debug, Clone)]
pub struct ToolInstallInfo {
    /// The tool name (as specified in jarvy.toml)
    pub name: String,
    /// The version hint from configuration
    pub version: String,
    /// The package manager to use
    pub package_manager: PackageManager,
    /// The package name for this package manager
    pub package_name: String,
}

/// Categorization of tools for installation.
#[derive(Debug, Default)]
pub struct ToolGroups {
    /// Tools grouped by package manager (package_manager -> list of (tool_name, package_name, version))
    pub by_package_manager: std::collections::HashMap<PackageManager, Vec<(String, String, String)>>,
    /// Tools with custom installers that must run individually
    pub custom_install: Vec<(String, String)>,
    /// Tools not in the registry (unknown)
    pub unknown: Vec<(String, String)>,
}

impl ToolGroups {
    /// Check if there are any tools to install via package managers.
    pub fn has_package_manager_tools(&self) -> bool {
        !self.by_package_manager.is_empty()
    }

    /// Get the total count of all tools.
    pub fn total_count(&self) -> usize {
        let pm_count: usize = self.by_package_manager.values().map(|v| v.len()).sum();
        pm_count + self.custom_install.len() + self.unknown.len()
    }
}

/// Get the package manager and package name for a tool on the current platform.
///
/// Returns None if:
/// - The tool is not in the registry
/// - The tool has no installation defined for the current platform
/// - The tool uses a custom installer
pub fn get_tool_install_info(tool_name: &str, version: &str) -> Option<ToolInstallInfo> {
    let spec = get_tool_spec(tool_name)?;

    // Skip tools with custom installers
    if spec.custom_install.is_some() {
        return None;
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(macos) = spec.macos {
            // Prefer cask for GUI apps
            if let Some(cask_name) = macos.cask {
                return Some(ToolInstallInfo {
                    name: tool_name.to_string(),
                    version: version.to_string(),
                    package_manager: PackageManager::BrewCask,
                    package_name: cask_name.to_string(),
                });
            }
            if let Some(brew_name) = macos.brew {
                return Some(ToolInstallInfo {
                    name: tool_name.to_string(),
                    version: version.to_string(),
                    package_manager: PackageManager::Brew,
                    package_name: brew_name.to_string(),
                });
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(linux) = spec.linux {
            // Detect the system package manager
            if let Some(pm) = super::common::detect_linux_pm() {
                if let Some(pkg_name) = linux.get(pm) {
                    return Some(ToolInstallInfo {
                        name: tool_name.to_string(),
                        version: version.to_string(),
                        package_manager: pm,
                        package_name: pkg_name.to_string(),
                    });
                }
            }
            // Fallback to Linuxbrew
            if let Some(brew_name) = linux.brew {
                if super::common::has("brew") {
                    return Some(ToolInstallInfo {
                        name: tool_name.to_string(),
                        version: version.to_string(),
                        package_manager: PackageManager::Brew,
                        package_name: brew_name.to_string(),
                    });
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(windows) = spec.windows {
            // Prefer winget
            if let Some(winget_id) = windows.winget {
                if super::common::has("winget") {
                    return Some(ToolInstallInfo {
                        name: tool_name.to_string(),
                        version: version.to_string(),
                        package_manager: PackageManager::Winget,
                        package_name: winget_id.to_string(),
                    });
                }
            }
            // Fallback to Chocolatey
            if let Some(choco_name) = windows.choco {
                if super::common::has("choco") {
                    return Some(ToolInstallInfo {
                        name: tool_name.to_string(),
                        version: version.to_string(),
                        package_manager: PackageManager::Choco,
                        package_name: choco_name.to_string(),
                    });
                }
            }
        }
    }

    None
}

/// Check if a tool has a custom installer.
pub fn has_custom_installer(tool_name: &str) -> bool {
    get_tool_spec(tool_name)
        .map(|spec| spec.custom_install.is_some())
        .unwrap_or(false)
        || MANUAL_TOOLS.iter().any(|(name, _)| *name == tool_name.to_lowercase())
}

/// Group a list of tools by their installation method.
///
/// This separates tools into:
/// - Tools installable via package manager (grouped by PM)
/// - Tools with custom installers
/// - Unknown tools (not in registry)
///
/// # Arguments
/// * `tools` - Iterator of (tool_name, version) tuples
pub fn group_tools_for_installation<'a, I>(tools: I) -> ToolGroups
where
    I: Iterator<Item = (&'a str, &'a str)>,
{
    let mut groups = ToolGroups::default();

    for (name, version) in tools {
        let name_lower = name.to_lowercase();

        // Check if it's a known tool
        let is_known = get_tool_spec(&name_lower).is_some()
            || MANUAL_TOOLS.iter().any(|(n, _)| *n == name_lower);

        if !is_known {
            groups.unknown.push((name.to_string(), version.to_string()));
            continue;
        }

        // Check for custom installer
        if has_custom_installer(&name_lower) {
            groups.custom_install.push((name.to_string(), version.to_string()));
            continue;
        }

        // Try to get package manager info
        if let Some(info) = get_tool_install_info(&name_lower, version) {
            groups
                .by_package_manager
                .entry(info.package_manager)
                .or_default()
                .push((info.name, info.package_name, info.version));
        } else {
            // No PM info available (e.g., platform not supported), treat as custom
            groups.custom_install.push((name.to_string(), version.to_string()));
        }
    }

    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test ToolSpec with all fields
    static TEST_TOOL: ToolSpec = ToolSpec {
        name: "test",
        command: "test_cmd",
        macos: Some(MacOsInstall {
            brew: Some("test"),
            cask: None,
        }),
        linux: Some(LinuxInstall::uniform("test")),
        windows: Some(WindowsInstall {
            winget: Some("Test.Test"),
            choco: Some("test"),
        }),
        custom_install: None,
        default_hook: None,
    };

    // Test ToolSpec with a default hook
    static TEST_TOOL_WITH_HOOK: ToolSpec = ToolSpec {
        name: "test_hooked",
        command: "test_hooked_cmd",
        macos: Some(MacOsInstall::brew("test")),
        linux: Some(LinuxInstall::uniform("test")),
        windows: None,
        custom_install: None,
        default_hook: Some(DefaultHook::new(
            "Configure test tool",
            "echo 'test hook executed'",
        )),
    };

    #[test]
    fn test_tool_spec_fields() {
        assert_eq!(TEST_TOOL.name, "test");
        assert_eq!(TEST_TOOL.command, "test_cmd");
        assert!(TEST_TOOL.macos.is_some());
        assert!(TEST_TOOL.linux.is_some());
        assert!(TEST_TOOL.windows.is_some());
    }

    #[test]
    fn test_linux_install_uniform() {
        let linux = LinuxInstall::uniform("git");
        assert_eq!(linux.apt, Some("git"));
        assert_eq!(linux.dnf, Some("git"));
        assert_eq!(linux.pacman, Some("git"));
    }

    #[test]
    fn test_linux_install_get() {
        let linux = LinuxInstall::uniform("git");
        assert_eq!(linux.get(PackageManager::Apt), Some("git"));
        assert_eq!(linux.get(PackageManager::Dnf), Some("git"));
        assert_eq!(linux.get(PackageManager::Brew), None); // uniform() doesn't set brew
    }

    #[test]
    fn test_linux_install_brew() {
        let linux = LinuxInstall::brew("upbound/tap/up");
        assert_eq!(linux.brew, Some("upbound/tap/up"));
        assert_eq!(linux.apt, None);
        assert_eq!(linux.get(PackageManager::Brew), Some("upbound/tap/up"));
    }

    #[test]
    fn test_macos_install_helpers() {
        let brew = MacOsInstall::brew("git");
        assert_eq!(brew.brew, Some("git"));
        assert_eq!(brew.cask, None);

        let cask = MacOsInstall::cask("docker");
        assert_eq!(cask.brew, None);
        assert_eq!(cask.cask, Some("docker"));
    }

    #[test]
    fn test_windows_install_helpers() {
        let winget = WindowsInstall::winget("Git.Git");
        assert_eq!(winget.winget, Some("Git.Git"));
        assert_eq!(winget.choco, None);

        let choco = WindowsInstall::choco("git");
        assert_eq!(choco.winget, None);
        assert_eq!(choco.choco, Some("git"));
    }

    // Test that is_satisfied returns false for non-existent command
    #[test]
    fn test_is_satisfied_nonexistent() {
        let tool = ToolSpec {
            name: "nonexistent",
            command: "definitely_not_a_real_command_xyz",
            macos: None,
            linux: None,
            windows: None,
            custom_install: None,
            default_hook: None,
        };
        assert!(!tool.is_satisfied("1.0"));
    }

    // ========================================================================
    // Tool Index Tests
    // ========================================================================

    #[test]
    fn test_tool_index_entry_from_spec() {
        let entry = ToolIndexEntry::from(&TEST_TOOL);
        assert_eq!(entry.name, "test");
        assert_eq!(entry.command, "test_cmd");
        assert!(entry.macos.is_some());
        assert!(entry.linux.is_some());
        assert!(entry.windows.is_some());
        assert!(!entry.custom_install.has_custom_installer);
    }

    #[test]
    fn test_tool_index_entry_with_custom_installer() {
        let custom_tool = ToolSpec {
            name: "custom",
            command: "custom_cmd",
            macos: None,
            linux: None,
            windows: None,
            custom_install: Some(|_| Ok(())),
            default_hook: None,
        };
        let entry = ToolIndexEntry::from(&custom_tool);
        assert!(entry.custom_install.has_custom_installer);
    }

    #[test]
    fn test_generate_tool_index_has_tools() {
        let index = generate_tool_index();
        // Should have at least the 3 manual tools (nvm, rust, brew)
        assert!(index.count >= 3, "Expected at least 3 tools, got {}", index.count);
        assert_eq!(index.tools.len(), index.count);
    }

    #[test]
    fn test_generate_tool_index_version() {
        let index = generate_tool_index();
        assert_eq!(index.version, ToolIndex::VERSION);
    }

    #[test]
    fn test_generate_tool_index_sorted() {
        let index = generate_tool_index();
        let names: Vec<&str> = index.tools.iter().map(|t| t.name.as_str()).collect();
        let mut sorted_names = names.clone();
        sorted_names.sort();
        assert_eq!(names, sorted_names, "Tool index should be sorted by name");
    }

    #[test]
    fn test_generate_tool_index_contains_manual_tools() {
        let index = generate_tool_index();
        let names: Vec<&str> = index.tools.iter().map(|t| t.name.as_str()).collect();

        // Manual tools should be present
        assert!(names.contains(&"nvm"), "Should contain nvm");
        assert!(names.contains(&"rust"), "Should contain rust");
        assert!(names.contains(&"brew"), "Should contain brew");
    }

    #[test]
    fn test_generate_tool_index_json_valid() {
        let json = generate_tool_index_json();
        // Should be valid JSON
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&json);
        assert!(parsed.is_ok(), "Generated JSON should be valid: {}", json);

        let value = parsed.unwrap();
        assert!(value.get("version").is_some());
        assert!(value.get("count").is_some());
        assert!(value.get("tools").is_some());
    }

    #[test]
    fn test_list_tool_names_not_empty() {
        let names = list_tool_names();
        assert!(!names.is_empty(), "Tool names list should not be empty");
    }

    #[test]
    fn test_list_tool_names_sorted() {
        let names = list_tool_names();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "Tool names should be sorted");
    }

    #[test]
    fn test_list_tool_names_contains_manual_tools() {
        let names = list_tool_names();
        assert!(names.contains(&"nvm".to_string()), "Should contain nvm");
        assert!(names.contains(&"rust".to_string()), "Should contain rust");
        assert!(names.contains(&"brew".to_string()), "Should contain brew");
    }

    #[test]
    fn test_tool_spec_serialization() {
        let json = serde_json::to_string(&TEST_TOOL);
        assert!(json.is_ok(), "ToolSpec should serialize to JSON");

        let json_str = json.unwrap();
        assert!(json_str.contains("\"name\":\"test\""));
        assert!(json_str.contains("\"command\":\"test_cmd\""));
        // custom_install should be skipped
        assert!(!json_str.contains("custom_install"));
    }

    #[test]
    fn test_tool_index_serialization() {
        let index = generate_tool_index();
        let json = serde_json::to_string_pretty(&index);
        assert!(json.is_ok(), "ToolIndex should serialize to JSON");

        let json_str = json.unwrap();
        assert!(json_str.contains("\"version\""));
        assert!(json_str.contains("\"count\""));
        assert!(json_str.contains("\"tools\""));
    }

    // ========================================================================
    // Default Hook Tests
    // ========================================================================

    #[test]
    fn test_default_hook_new() {
        let hook = DefaultHook::new("Test hook", "echo test");
        assert_eq!(hook.description, "Test hook");
        assert_eq!(hook.script, "echo test");
        assert!(hook.platform.is_none());
    }

    #[test]
    fn test_default_hook_for_platform() {
        let hook = DefaultHook::for_platform("macOS only", "brew info", "macos");
        assert_eq!(hook.description, "macOS only");
        assert_eq!(hook.script, "brew info");
        assert_eq!(hook.platform, Some("macos"));
    }

    #[test]
    fn test_default_hook_should_run_no_platform() {
        let hook = DefaultHook::new("All platforms", "echo hello");
        // Should always return true when platform is None
        assert!(hook.should_run_on_current_platform());
    }

    #[test]
    fn test_default_hook_should_run_current_platform() {
        // Test that current platform's hook returns true
        #[cfg(target_os = "macos")]
        {
            let hook = DefaultHook::for_platform("macOS hook", "ls", "macos");
            assert!(hook.should_run_on_current_platform());
            let other = DefaultHook::for_platform("Linux hook", "ls", "linux");
            assert!(!other.should_run_on_current_platform());
        }
        #[cfg(target_os = "linux")]
        {
            let hook = DefaultHook::for_platform("Linux hook", "ls", "linux");
            assert!(hook.should_run_on_current_platform());
            let other = DefaultHook::for_platform("macOS hook", "ls", "macos");
            assert!(!other.should_run_on_current_platform());
        }
        #[cfg(target_os = "windows")]
        {
            let hook = DefaultHook::for_platform("Windows hook", "dir", "windows");
            assert!(hook.should_run_on_current_platform());
            let other = DefaultHook::for_platform("Linux hook", "ls", "linux");
            assert!(!other.should_run_on_current_platform());
        }
    }

    #[test]
    fn test_default_hook_unknown_platform() {
        let hook = DefaultHook::for_platform("Unknown", "test", "bsd");
        // Unknown platforms should return false
        assert!(!hook.should_run_on_current_platform());
    }

    #[test]
    fn test_tool_spec_get_default_hook() {
        // Tool with no hook
        assert!(TEST_TOOL.get_default_hook().is_none());
        assert!(!TEST_TOOL.has_default_hook());

        // Tool with hook
        let hook = TEST_TOOL_WITH_HOOK.get_default_hook();
        assert!(hook.is_some());
        assert!(TEST_TOOL_WITH_HOOK.has_default_hook());
        let h = hook.unwrap();
        assert_eq!(h.description, "Configure test tool");
        assert_eq!(h.script, "echo 'test hook executed'");
    }

    #[test]
    fn test_tool_spec_with_platform_hook() {
        // Create a tool with a platform-specific hook
        static PLATFORM_TOOL: ToolSpec = ToolSpec {
            name: "platform_test",
            command: "platform_cmd",
            macos: None,
            linux: None,
            windows: None,
            custom_install: None,
            #[cfg(target_os = "macos")]
            default_hook: Some(DefaultHook::for_platform("macOS hook", "brew info", "macos")),
            #[cfg(target_os = "linux")]
            default_hook: Some(DefaultHook::for_platform("Linux hook", "apt info", "linux")),
            #[cfg(target_os = "windows")]
            default_hook: Some(DefaultHook::for_platform("Windows hook", "winget info", "windows")),
        };

        // Hook should be available on current platform
        assert!(PLATFORM_TOOL.has_default_hook());
    }

    #[test]
    fn test_default_hook_serialization() {
        let hook = DefaultHook::new("Test", "echo test");
        let json = serde_json::to_string(&hook);
        assert!(json.is_ok());
        let json_str = json.unwrap();
        assert!(json_str.contains("\"description\":\"Test\""));
        assert!(json_str.contains("\"script\":\"echo test\""));
        // platform should be skipped when None
        assert!(!json_str.contains("platform"));
    }

    #[test]
    fn test_default_hook_with_platform_serialization() {
        let hook = DefaultHook::for_platform("macOS", "ls", "macos");
        let json = serde_json::to_string(&hook);
        assert!(json.is_ok());
        let json_str = json.unwrap();
        assert!(json_str.contains("\"platform\":\"macos\""));
    }

    #[test]
    fn test_tool_groups_default() {
        let groups = ToolGroups::default();
        assert!(groups.by_package_manager.is_empty());
        assert!(groups.custom_install.is_empty());
        assert!(groups.unknown.is_empty());
    }

    #[test]
    fn test_tool_install_info_struct() {
        use crate::tools::common::PackageManager;
        let info = ToolInstallInfo {
            name: "jq".to_string(),
            version: "latest".to_string(),
            package_manager: PackageManager::Brew,
            package_name: "jq".to_string(),
        };
        assert_eq!(info.name, "jq");
        assert_eq!(info.version, "latest");
        assert_eq!(info.package_manager, PackageManager::Brew);
        assert_eq!(info.package_name, "jq");
    }

    #[test]
    fn test_group_tools_empty() {
        let tools: Vec<(&str, &str)> = vec![];
        let groups = group_tools_for_installation(tools.into_iter());
        assert!(groups.by_package_manager.is_empty());
        assert!(groups.custom_install.is_empty());
        assert!(groups.unknown.is_empty());
    }

    #[test]
    fn test_group_tools_with_unknown() {
        let tools = vec![("nonexistent_tool_xyz", "1.0")];
        let groups = group_tools_for_installation(tools.into_iter());
        // Unknown tools should be in the unknown list
        assert_eq!(groups.unknown.len(), 1);
        assert_eq!(groups.unknown[0].0, "nonexistent_tool_xyz");
    }

    #[test]
    fn test_has_custom_installer_known_tools() {
        // brew has a custom installer (in MANUAL_TOOLS)
        assert!(has_custom_installer("brew"));
        // rust has a custom installer (in MANUAL_TOOLS)
        assert!(has_custom_installer("rust"));
        // nvm has a custom installer (in MANUAL_TOOLS)
        assert!(has_custom_installer("nvm"));
        // jq does not have a custom installer
        assert!(!has_custom_installer("jq"));
        // git does not have a custom installer
        assert!(!has_custom_installer("git"));
    }
}
