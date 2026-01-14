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
#[derive(Debug, Clone, Copy)]
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
#[derive(Debug, Clone, Copy)]
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
#[derive(Debug, Clone, Copy)]
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

/// Declarative tool specification that eliminates boilerplate.
///
/// A `ToolSpec` defines everything needed to check for and install a tool
/// across all supported platforms. The `ensure()` method handles the common
/// pattern of checking version satisfaction and installing if needed.
#[derive(Debug, Clone, Copy)]
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
    pub custom_install: Option<CustomInstallFn>,
}

impl ToolSpec {
    /// Check if the tool is installed and satisfies the version requirement.
    pub fn is_satisfied(&self, min_hint: &str) -> bool {
        cmd_satisfies(self.command, min_hint)
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
    }) => {
        pub static $name: $crate::tools::spec::ToolSpec = $crate::tools::spec::ToolSpec {
            name: stringify!($name),
            command: $cmd,
            macos: define_tool!(@macos $($($macos_key: $macos_val),*)?),
            linux: define_tool!(@linux $($($linux_key: $linux_val),*)?),
            windows: define_tool!(@windows $($($windows_key: $windows_val),*)?),
            custom_install: define_tool!(@custom $($custom)?),
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
}

pub use define_tool;

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
        };
        assert!(!tool.is_satisfied("1.0"));
    }
}
