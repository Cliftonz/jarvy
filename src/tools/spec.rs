//! Declarative tool specification pattern for reducing boilerplate.
//!
//! This module provides the `ToolSpec` struct and `define_tool!` macro that
//! eliminates ~80% of code duplication across tool implementations.

#![allow(dead_code)] // Public API for tool specification and installation
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

use super::common::{InstallError, PackageManager, cmd_satisfies, has, run};

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
use super::common::{PkgOps, default_use_sudo};

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

/// BSD installation options (initially FreeBSD with pkg).
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct BsdInstall {
    /// FreeBSD pkg package name (e.g., "git", "jq")
    pub pkg: Option<&'static str>,
}

impl BsdInstall {
    /// Create a BsdInstall with a pkg package name.
    pub const fn pkg(name: &'static str) -> Self {
        Self { pkg: Some(name) }
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
            Some("bsd") | Some("freebsd") => cfg!(target_os = "freebsd"),
            // POSIX-shell hooks on tools that also install on Windows —
            // hooks run under PowerShell there, so `[ ... ]`/`$HOME`
            // scripts must skip rather than fail every setup with
            // advisory warnings.
            Some("unix") => cfg!(unix),
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

    /// BSD installation options (None if not supported on BSD/FreeBSD)
    pub bsd: Option<BsdInstall>,

    /// Optional custom install function for complex tools (nvm, rustup, etc.)
    /// If provided, this takes precedence over standard package manager installs.
    #[serde(skip)]
    pub custom_install: Option<CustomInstallFn>,

    /// Optional default post-install hook that runs after tool installation.
    /// Used for shell integration, PATH setup, config generation, etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_hook: Option<DefaultHook>,

    /// Optional list of tool names that must be installed before this tool.
    /// Used for dependency ordering (e.g., node depends on nvm, python depends on pyenv).
    /// This is a STRICT dependency - ALL listed tools must be available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<&'static [&'static str]>,

    /// Optional list of tool names where AT LEAST ONE must be available.
    /// Used for flexible dependencies where multiple tools can satisfy the requirement.
    /// Example: kubectl can work with minikube, kind, docker, or any K8s cluster provider.
    /// If one is already installed, the dependency is satisfied.
    /// If none is installed but one is in config, that one will be installed first.
    /// If none is installed or in config, a warning is shown but installation proceeds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on_one_of: Option<&'static [&'static str]>,

    /// Optional category for filtering and organization (e.g., "devops", "language", "editor").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<&'static str>,
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
        #[cfg(target_os = "freebsd")]
        {
            return self.install_bsd();
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
            // Recent Homebrew versions refuse third-party tap formulae
            // unless the tap was previously trusted via `brew tap`. The
            // formula identifier `org/tap/name` carries exactly two
            // slashes; auto-tap when we see that shape so a fresh box
            // doesn't surface a confusing "untrusted tap" error.
            if formula.matches('/').count() == 2 {
                let tap_path: String = formula.split('/').take(2).collect::<Vec<_>>().join("/");
                // Soft-fail: a non-zero `brew tap` exit means the tap is
                // already added or the network is unreachable — either
                // way, let the subsequent `brew install` produce the
                // canonical error message.
                let _ = run("brew", &["tap", &tap_path]);
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
                // Same auto-tap behavior as the macOS path — see
                // `install_macos` for the rationale.
                if brew_pkg.matches('/').count() == 2 {
                    let tap_path: String =
                        brew_pkg.split('/').take(2).collect::<Vec<_>>().join("/");
                    let _ = run("brew", &["tap", &tap_path]);
                }
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

    #[cfg(target_os = "freebsd")]
    fn install_bsd(&self) -> Result<(), InstallError> {
        use super::common::{PkgOps, default_use_sudo};

        let bsd = self.bsd.ok_or(InstallError::Unsupported)?;

        if let Some(pkg_name) = bsd.pkg {
            if let Some(pm) = super::common::detect_bsd_pm() {
                let _ = PkgOps::update(pm, default_use_sudo());
                return PkgOps::install(pm, pkg_name, default_use_sudo());
            }
        }

        Err(InstallError::Prereq(
            "No supported BSD package manager on PATH (pkg)",
        ))
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
///
/// // With strict dependencies (ALL must be installed):
/// define_tool!(node, {
///     command: "node",
///     macos: { brew: "node" },
///     linux: { uniform: "nodejs" },
///     windows: { winget: "OpenJS.NodeJS" },
///     depends_on: &["nvm"],  // nvm must be installed before node
/// });
///
/// // With flexible dependencies (ONE OF must be installed):
/// define_tool!(kubectl, {
///     command: "kubectl",
///     macos: { brew: "kubectl" },
///     linux: { uniform: "kubectl" },
///     windows: { winget: "Kubernetes.kubectl" },
///     depends_on_one_of: &["minikube", "kind", "docker"],  // needs any K8s cluster
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
        $(bsd: { $($bsd_key:ident: $bsd_val:expr),* $(,)? },)?
        $(custom_install: $custom:expr,)?
        $(default_hook: { description: $hook_desc:expr, script: $hook_script:expr $(, platform: $hook_platform:expr)? },)?
        $(default_hook_shell_init: ($shell_tool:literal, $shell_verb:literal),)?
        $(depends_on: $deps:expr,)?
        $(depends_on_one_of: $flex_deps:expr,)?
        $(category: $category:expr,)?
    }) => {
        pub static $name: $crate::tools::spec::ToolSpec = $crate::tools::spec::ToolSpec {
            name: stringify!($name),
            command: $cmd,
            macos: define_tool!(@macos $($($macos_key: $macos_val),*)?),
            linux: define_tool!(@linux $($($linux_key: $linux_val),*)?),
            windows: define_tool!(@windows $($($windows_key: $windows_val),*)?),
            bsd: define_tool!(@bsd $($($bsd_key: $bsd_val),*)?),
            custom_install: define_tool!(@custom $($custom)?),
            default_hook: define_tool!(
                @resolve_default_hook
                $($hook_desc, $hook_script $(, $hook_platform)?)?
                ;
                $(($shell_tool, $shell_verb))?
            ),
            depends_on: define_tool!(@depends_on $($deps)?),
            depends_on_one_of: define_tool!(@depends_on_one_of $($flex_deps)?),
            category: define_tool!(@category $($category)?),
        };

        #[allow(dead_code)] // Public API for tool installation
        pub fn ensure(min_hint: &str) -> Result<(), $crate::tools::common::InstallError> {
            $name.ensure(min_hint)
        }

        #[allow(dead_code)] // Used by inventory submission
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

    // Resolve default_hook from EITHER the explicit { description, script }
    // form OR the shell_init shorthand. Exactly one (or neither) must be set.
    (@resolve_default_hook ;) => { None };
    (@resolve_default_hook $desc:expr, $script:expr ;) => {
        Some($crate::tools::spec::DefaultHook::new($desc, $script))
    };
    (@resolve_default_hook $desc:expr, $script:expr, $platform:expr ;) => {
        Some($crate::tools::spec::DefaultHook::for_platform($desc, $script, $platform))
    };
    (@resolve_default_hook ; ($tool:literal, $verb:literal)) => {
        define_tool!(@shell_init_hook $tool, $verb)
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
    // Linuxbrew with Alpine apk support (for tools available in Alpine but not other distros)
    (@linux brew: $brew:expr, apk: $apk:expr) => {
        Some($crate::tools::spec::LinuxInstall {
            apt: None,
            dnf: None,
            yum: None,
            zypper: None,
            pacman: None,
            apk: Some($apk),
            brew: Some($brew),
        })
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

    // BSD helpers
    (@bsd) => { None };
    (@bsd pkg: $val:expr) => {
        Some($crate::tools::spec::BsdInstall::pkg($val))
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

    // Shell-init shorthand: `default_hook_shell_init: ("starship", "init"),`
    // expands to the canonical "append eval $(<tool> <verb> <shell>) to
    // .bashrc / .zshrc if not present" body. Replaces the 33 byte-identical
    // hand-rolled scripts that previously lived in tool definitions
    // (maintainability review F-5).
    //
    // Both args MUST be string literals so `concat!` can evaluate at compile
    // time — DefaultHook::script demands `&'static str`.
    (@shell_init_hook $tool:literal, $verb:literal) => {
        Some($crate::tools::spec::DefaultHook::new(
            concat!("Add ", $tool, " shell initialization to .bashrc and .zshrc"),
            concat!(
                "\n# ", $tool, " shell integration\n",
                "INIT_BASH='eval \"$(", $tool, " ", $verb, " bash)\"'\n",
                "INIT_ZSH='eval \"$(", $tool, " ", $verb, " zsh)\"'\n",
                "\n",
                "if [ -f \"$HOME/.bashrc\" ] && ! grep -q '", $tool, " ", $verb, " bash' \"$HOME/.bashrc\"; then\n",
                "    echo \"$INIT_BASH\" >> \"$HOME/.bashrc\"\n",
                "    echo \"Added ", $tool, " init to ~/.bashrc\"\n",
                "fi\n",
                "\n",
                "if [ -f \"$HOME/.zshrc\" ] && ! grep -q '", $tool, " ", $verb, " zsh' \"$HOME/.zshrc\"; then\n",
                "    echo \"$INIT_ZSH\" >> \"$HOME/.zshrc\"\n",
                "    echo \"Added ", $tool, " init to ~/.zshrc\"\n",
                "fi\n"
            ),
        ))
    };

    // Strict dependency helpers
    (@depends_on) => { None };
    (@depends_on $deps:expr) => { Some($deps) };

    // Flexible dependency helpers (one-of)
    (@depends_on_one_of) => { None };
    (@depends_on_one_of $deps:expr) => { Some($deps) };

    // Category helper
    (@category) => { None };
    (@category $cat:expr) => { Some($cat) };
}

#[allow(unused_imports)]
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

/// Default-hook metadata surfaced in the tool index. Only the description
/// and platform filter are exported — the script body stays out of the
/// index so consumers (docs generator, MCP) don't ship shell source.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DefaultHookInfo {
    /// Human-readable description of what the hook does.
    pub description: String,
    /// Platform filter ("macos", "linux", "windows", "unix"), if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
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
    /// BSD installation options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bsd: Option<BsdInstall>,
    /// Custom installation info
    pub custom_install: CustomInstallInfo,
    /// Tool category for filtering (e.g., "devops", "language", "editor")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Strict dependencies — ALL must be installed before this tool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,
    /// Flexible dependencies — at least ONE must be available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on_one_of: Option<Vec<String>>,
    /// Default post-install hook metadata (description only, no script body)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_hook: Option<DefaultHookInfo>,
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
            bsd: spec.bsd,
            custom_install: CustomInstallInfo {
                has_custom_installer: spec.custom_install.is_some(),
            },
            category: spec.category.map(|s| s.to_string()),
            depends_on: spec
                .depends_on
                .map(|d| d.iter().map(|s| s.to_string()).collect()),
            depends_on_one_of: spec
                .depends_on_one_of
                .map(|d| d.iter().map(|s| s.to_string()).collect()),
            default_hook: spec.default_hook.as_ref().map(|h| DefaultHookInfo {
                description: h.description.to_string(),
                platform: h.platform.map(str::to_string),
            }),
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
    /// 1.1.0: added `depends_on`, `depends_on_one_of`, `default_hook`
    /// (description + platform only) to entries — additive.
    pub const VERSION: &'static str = "1.1.0";
}

/// Manually registered tools that don't use the `define_tool!` macro.
/// These tools have custom installation logic and are registered in `register_all()`.
/// (`rust` migrated to `define_tool!` + `custom_install`, so it is now
/// collected via `iter_tools()` — listing it here too produced a
/// duplicate, null-platform index entry, QA review F4.)
const MANUAL_TOOLS: &[(&str, &str)] = &[("nvm", "nvm"), ("brew", "brew")];

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
            bsd: None,
            custom_install: CustomInstallInfo {
                has_custom_installer: true,
            },
            category: None,
            depends_on: None,
            depends_on_one_of: None,
            default_hook: None,
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
    serde_json::to_string_pretty(&index).unwrap_or_else(|e| format!(r#"{{"error": "{}"}}"#, e))
}

/// Borrowed iteration over every registered tool name, sorted and
/// canonical-lowercase.
///
/// The underlying `Vec<String>` is built once via `LazyLock` and reused
/// for every call — replacing the per-call rebuild in
/// `list_tool_names` for hot paths like `unsupported::fuzzy_suggest`.
///
/// Names are lowercased at cache-build time so the inventory can
/// include conventionally-uppercase entries (e.g. acronyms like
/// `kMCP`) without breaking case-insensitive lookups downstream. The
/// cost is ~150 small `String` allocations exactly once per process,
/// amortized across every subsequent call.
pub fn iter_tool_names() -> impl Iterator<Item = &'static str> {
    static SORTED_NAMES: std::sync::LazyLock<Vec<String>> = std::sync::LazyLock::new(|| {
        let mut v: Vec<String> = iter_tools()
            .map(|e| e.spec.name.to_ascii_lowercase())
            .collect();
        v.extend(MANUAL_TOOLS.iter().map(|(n, _)| (*n).to_ascii_lowercase()));
        v.sort();
        v.dedup();
        v
    });
    // `SORTED_NAMES` has `'static` storage via `LazyLock`, so the
    // `&String` it yields is `&'static String` and `as_str()` is
    // therefore `&'static str` — no lifetime bound to a borrow scope.
    SORTED_NAMES.iter().map(String::as_str)
}

/// Get a list of all supported tool names (lowercase). Owned-string
/// shape kept for callers that need `Vec<String>`; new code should
/// prefer `iter_tool_names()` to avoid rebuilding the vector.
pub fn list_tool_names() -> Vec<String> {
    iter_tool_names().map(str::to_string).collect()
}

/// Look up the `category:` field set on a tool's `ToolSpec`. Returns
/// `None` when the tool isn't in the registry or its spec doesn't
/// set a category. Used by telemetry call sites so `tool.installed`
/// / `tool.failed` events carry a category label without forcing
/// every event emission to know the tool ontology.
pub fn get_tool_category(name: &str) -> Option<&'static str> {
    let lower = name.to_ascii_lowercase();
    iter_tools()
        .find(|e| e.spec.name.eq_ignore_ascii_case(&lower))
        .and_then(|e| e.spec.category)
}

/// Render the canonical `define_tool!` template — re-exported from
/// the dep-free `jarvy-templates` crate. Both `cargo-jarvy new-tool`
/// and `jarvy tools --request` call the same implementation, so the
/// drift that motivated this consolidation (missing `__PKG_BSD__`)
/// cannot recur.
pub use jarvy_templates::render_tool_template;

/// Look up a ToolSpec by name (case-insensitive).
/// Returns None if the tool is not found or is a manually registered tool.
///
/// Uses a `OnceLock<HashMap>` over the inventory so each lookup is O(1) and
/// allocation-free. Previously every call lowercased all ~150 spec names
/// per lookup; for a 30-tool config in `check_tools_parallel` that produced
/// ~4,500 transient `String` allocations during version checking alone.
pub fn get_tool_spec(name: &str) -> Option<&'static ToolSpec> {
    static SPECS_BY_LOWERCASE_NAME: std::sync::OnceLock<
        std::collections::HashMap<String, &'static ToolSpec>,
    > = std::sync::OnceLock::new();

    let map = SPECS_BY_LOWERCASE_NAME.get_or_init(|| {
        iter_tools()
            .map(|entry| (entry.spec.name.to_lowercase(), entry.spec))
            .collect()
    });

    // ASCII case-fold of the input is enough — tool names are ASCII.
    if name.is_empty() {
        return None;
    }
    let key_owned;
    let key: &str = if name
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b == b'-' || b.is_ascii_digit())
    {
        name
    } else {
        key_owned = name.to_ascii_lowercase();
        key_owned.as_str()
    };
    if let Some(spec) = map.get(key).copied() {
        return Some(spec);
    }
    // Tolerate the natural user form: every NATS doc shows
    // `nats-server` (hyphen) but `define_tool!(NATS_SERVER, ...)`
    // stringifies as `nats_server` (underscore). Without this fallback
    // `validate` would accept the hyphen form (it has its own aliasing
    // — see `commands::validate::validate_tools`) while
    // `check_tools_parallel` would emit `tool.unsupported` for the same
    // name — a user-visible divergence found during v0.2.0-rc.1 soak.
    if key.contains('-') {
        let alias = key.replace('-', "_");
        if let Some(spec) = map.get(&alias).copied() {
            return Some(spec);
        }
    }
    if key.contains('_') {
        let alias = key.replace('_', "-");
        if let Some(spec) = map.get(&alias).copied() {
            return Some(spec);
        }
    }
    None
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

    // Check if tool is known - look up spec and manual tool command in one pass
    let spec = get_tool_spec(&name_lower);
    let manual_cmd = MANUAL_TOOLS
        .iter()
        .find_map(|(n, cmd)| if *n == name_lower { Some(*cmd) } else { None });

    // If neither a registered tool nor a manual tool, it's unknown
    if spec.is_none() && manual_cmd.is_none() {
        return ToolVersionStatus {
            name: name.to_string(),
            version: version.to_string(),
            satisfied: false,
            known: false,
        };
    }

    // Check if version is satisfied
    let satisfied = match (spec, manual_cmd) {
        (Some(spec), _) => spec.is_satisfied(version),
        // nvm has no binary — it's a shell function; `has()` (PATH lookup)
        // is unconditionally false on POSIX, which marked nvm needs-install
        // on every run. Probe its filesystem marker instead.
        (None, Some(_)) if name_lower == "nvm" => crate::tools::nvm::is_installed(),
        (None, Some(cmd)) => has(cmd),
        // This case is unreachable due to the early return above
        (None, None) => false,
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
    pub by_package_manager:
        std::collections::HashMap<PackageManager, Vec<(String, String, String)>>,
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

    #[cfg(target_os = "freebsd")]
    {
        if let Some(bsd) = spec.bsd {
            if let Some(pkg_name) = bsd.pkg {
                if super::common::has("pkg") {
                    return Some(ToolInstallInfo {
                        name: tool_name.to_string(),
                        version: version.to_string(),
                        package_manager: PackageManager::Pkg,
                        package_name: pkg_name.to_string(),
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
        || MANUAL_TOOLS
            .iter()
            .any(|(name, _)| *name == tool_name.to_lowercase())
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
            groups
                .custom_install
                .push((name.to_string(), version.to_string()));
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
            groups
                .custom_install
                .push((name.to_string(), version.to_string()));
        }
    }

    groups
}

// ============================================================================
// Dependency Ordering
// ============================================================================

/// Get the strict dependencies of a tool by name.
/// Returns an empty slice if the tool has no dependencies or is not found.
pub fn get_tool_dependencies(tool_name: &str) -> &'static [&'static str] {
    get_tool_spec(tool_name)
        .and_then(|spec| spec.depends_on)
        .unwrap_or(&[])
}

/// Get the flexible dependencies (one-of) of a tool by name.
/// Returns an empty slice if the tool has no flexible dependencies or is not found.
pub fn get_tool_flexible_dependencies(tool_name: &str) -> &'static [&'static str] {
    get_tool_spec(tool_name)
        .and_then(|spec| spec.depends_on_one_of)
        .unwrap_or(&[])
}

/// Result of checking a tool's dependencies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyCheckResult {
    /// All dependencies are satisfied.
    Satisfied,
    /// Missing strict dependencies (ALL must be installed).
    MissingRequired(Vec<String>),
    /// No flexible dependency installed, but one is in config and will be installed.
    WillInstallFlexible(String),
    /// No flexible dependency installed or in config - advisory warning.
    MissingFlexible {
        /// What the tool needs (description).
        needed: &'static str,
        /// Available options.
        options: Vec<String>,
        /// Suggested option to install.
        suggestion: Option<String>,
    },
}

impl DependencyCheckResult {
    /// Check if the result indicates all dependencies are satisfied.
    pub fn is_satisfied(&self) -> bool {
        matches!(
            self,
            DependencyCheckResult::Satisfied | DependencyCheckResult::WillInstallFlexible(_)
        )
    }

    /// Check if there are missing required dependencies (blocking).
    pub fn has_missing_required(&self) -> bool {
        matches!(self, DependencyCheckResult::MissingRequired(_))
    }

    /// Check if there are missing flexible dependencies (advisory).
    pub fn has_missing_flexible(&self) -> bool {
        matches!(self, DependencyCheckResult::MissingFlexible { .. })
    }
}

/// Check the dependency status of a tool.
///
/// # Arguments
/// * `tool_name` - The name of the tool to check
/// * `config_tools` - Set of tool names in the current config
/// * `installed_tools` - Set of tool names already installed on the system
///
/// # Returns
/// A `DependencyCheckResult` indicating the dependency status.
/// Check if dependency warnings should be suppressed.
/// Returns true if JARVY_IGNORE_MISSING_DEPS is set to "1" or "true".
pub fn should_ignore_missing_deps() -> bool {
    std::env::var("JARVY_IGNORE_MISSING_DEPS")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false)
}

pub fn check_tool_dependencies(
    tool_name: &str,
    config_tools: &std::collections::HashSet<String>,
    installed_tools: &std::collections::HashSet<String>,
) -> DependencyCheckResult {
    let spec = match get_tool_spec(tool_name) {
        Some(s) => s,
        None => return DependencyCheckResult::Satisfied, // Unknown tool, no deps to check
    };

    // Check strict dependencies first (ALL must be present)
    if let Some(strict_deps) = spec.depends_on {
        let missing: Vec<String> = strict_deps
            .iter()
            .filter(|dep| {
                let dep_lower = dep.to_lowercase();
                !installed_tools.contains(&dep_lower) && !config_tools.contains(&dep_lower)
            })
            .map(|s| s.to_string())
            .collect();

        if !missing.is_empty() {
            return DependencyCheckResult::MissingRequired(missing);
        }
    }

    // Check flexible dependencies (ONE OF must be present)
    if let Some(flex_deps) = spec.depends_on_one_of {
        // Check if any is already installed
        let any_installed = flex_deps.iter().any(|dep| {
            let dep_lower = dep.to_lowercase();
            installed_tools.contains(&dep_lower)
        });

        if any_installed {
            return DependencyCheckResult::Satisfied;
        }

        // Check if any is in config (will be installed)
        let in_config: Vec<&str> = flex_deps
            .iter()
            .filter(|dep| {
                let dep_lower = dep.to_lowercase();
                config_tools.contains(&dep_lower)
            })
            .copied()
            .collect();

        if !in_config.is_empty() {
            return DependencyCheckResult::WillInstallFlexible(in_config[0].to_string());
        }

        // None installed or in config - advisory warning
        return DependencyCheckResult::MissingFlexible {
            needed: "one of the following",
            options: flex_deps.iter().map(|s| s.to_string()).collect(),
            suggestion: flex_deps.first().map(|s| s.to_string()),
        };
    }

    DependencyCheckResult::Satisfied
}

/// Order tools by their dependencies using topological sort.
///
/// Tools with dependencies will be placed after their dependencies.
/// If tool A depends on tool B, then B will appear before A in the result.
///
/// This handles both strict dependencies (depends_on - ALL must be present)
/// and flexible dependencies (depends_on_one_of - if any is in the list, order appropriately).
///
/// # Arguments
/// * `tools` - Iterator of (tool_name, version) tuples
///
/// # Returns
/// A Vec of (tool_name, version) tuples ordered by dependencies.
/// If dependencies are missing from the input, they are NOT automatically added.
pub fn order_tools_by_dependencies<'a, I>(tools: I) -> Vec<(String, String)>
where
    I: Iterator<Item = (&'a str, &'a str)>,
{
    use std::collections::{HashMap, HashSet, VecDeque};

    let tool_list: Vec<(String, String)> = tools
        .map(|(n, v)| (n.to_lowercase(), v.to_string()))
        .collect();

    // Build a set of tools we're installing (for dependency filtering)
    let tool_set: HashSet<String> = tool_list.iter().map(|(n, _)| n.clone()).collect();

    // Build adjacency list (tool -> tools that depend on it)
    // and in-degree map (tool -> count of dependencies within our set).
    // Owned `String` keys — previous code leaked a `String` per edge via `.leak()`,
    // which is unsound for repeated invocations (long-lived processes / IDE plugins).
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();
    let mut in_degree: HashMap<String, usize> = HashMap::new();

    for (name, _) in &tool_list {
        in_degree.entry(name.clone()).or_insert(0);

        // Handle strict dependencies (ALL must be present)
        let deps = get_tool_dependencies(name);
        for dep in deps {
            let dep_lower = dep.to_lowercase();
            // Only count dependencies that are in our tool set
            if tool_set.contains(&dep_lower) {
                *in_degree.entry(name.clone()).or_insert(0) += 1;
                dependents.entry(dep_lower).or_default().push(name.clone());
            }
        }

        // Handle flexible dependencies (ONE OF - pick first match in our set)
        let flex_deps = get_tool_flexible_dependencies(name);
        if !flex_deps.is_empty() {
            // Find the first flexible dependency that is in our tool set
            if let Some(flex_dep) = flex_deps
                .iter()
                .find(|dep| tool_set.contains(&dep.to_lowercase()))
            {
                let dep_lower = flex_dep.to_lowercase();
                // Add edge: flex_dep -> name (flex_dep must be installed before name)
                *in_degree.entry(name.clone()).or_insert(0) += 1;
                dependents.entry(dep_lower).or_default().push(name.clone());
            }
        }
    }

    // Create a map from tool name to version for lookup
    let version_map: HashMap<&str, &str> = tool_list
        .iter()
        .map(|(n, v)| (n.as_str(), v.as_str()))
        .collect();

    // Kahn's algorithm for topological sort
    let mut queue: VecDeque<String> = in_degree
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(name, _)| name.clone())
        .collect();

    let mut result: Vec<(String, String)> = Vec::with_capacity(tool_list.len());

    while let Some(tool) = queue.pop_front() {
        if let Some(&version) = version_map.get(tool.as_str()) {
            result.push((tool.clone(), version.to_string()));
        }

        if let Some(deps) = dependents.get(&tool).cloned() {
            for dependent in deps {
                if let Some(deg) = in_degree.get_mut(&dependent) {
                    *deg = deg.saturating_sub(1);
                    if *deg == 0 {
                        queue.push_back(dependent);
                    }
                }
            }
        }
    }

    // If result is shorter than input, there's a cycle - return original order
    if result.len() != tool_list.len() {
        eprintln!("Warning: Circular dependency detected. Installing tools in original order.");
        return tool_list;
    }

    result
}

/// Check if a tool has any strict dependencies.
pub fn tool_has_dependencies(tool_name: &str) -> bool {
    !get_tool_dependencies(tool_name).is_empty()
}

/// Check if a tool has any flexible dependencies.
pub fn tool_has_flexible_dependencies(tool_name: &str) -> bool {
    !get_tool_flexible_dependencies(tool_name).is_empty()
}

/// Check if a tool has any dependencies (strict or flexible).
pub fn tool_has_any_dependencies(tool_name: &str) -> bool {
    tool_has_dependencies(tool_name) || tool_has_flexible_dependencies(tool_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: `validate` used to accept the natural hyphen form
    /// `nats-server` (because `commands::validate::validate_tools`
    /// applies dash↔underscore aliasing) while `setup`'s version-check
    /// path emitted `tool.unsupported` for the same name because
    /// `get_tool_spec` was strict on the registered identifier. Found
    /// during v0.2.0-rc.1 soak. Pin the aliasing here so the two
    /// paths stay in sync.
    #[test]
    fn get_tool_spec_tolerates_hyphen_underscore_aliasing() {
        // `nats_server` is registered as the underscore form.
        let underscore = get_tool_spec("nats_server");
        assert!(
            underscore.is_some(),
            "registered form must resolve directly"
        );
        // Hyphen form should resolve to the same spec.
        let hyphen = get_tool_spec("nats-server");
        assert!(
            hyphen.is_some(),
            "hyphen form must resolve via dash↔underscore aliasing"
        );
        // And both must point at the same ToolSpec.
        assert!(std::ptr::eq(
            underscore.unwrap() as *const _,
            hyphen.unwrap() as *const _
        ));
    }

    #[test]
    fn check_tool_version_resolves_hyphen_aliases() {
        // The end-to-end behavior: `check_tool_version("nats-server", ...)`
        // must return `known: true` so the setup path doesn't emit a
        // misleading `tool.unsupported` event.
        let status = check_tool_version("nats-server", "latest");
        assert!(
            status.known,
            "nats-server (hyphen) must be known via aliasing"
        );
    }

    // ----- render_tool_template direct coverage -----
    //
    // The previous test (`scaffold_snippet_matches_canonical_template`)
    // only asserted no `__PKG_BSD__` survived in the output — a
    // regression where BSD got replaced with empty string would have
    // passed. These tests pin the actual substituted values plus the
    // bin-override branch which had zero coverage.

    #[test]
    fn render_tool_template_substitutes_all_placeholders() {
        let out = render_tool_template("mytool", None);
        // No leftover placeholder must survive — generic check that
        // catches any future renamed/added placeholder we missed.
        assert!(
            !out.contains("__TOOL_") && !out.contains("__PKG_"),
            "unsubstituted placeholder leaked through: {}",
            out
        );
    }

    #[test]
    fn render_tool_template_substitutes_bsd_pkg_to_tool_name() {
        // The drift fix that motivated the consolidation. The previous
        // cargo-jarvy code missed `__PKG_BSD__` entirely.
        let out = render_tool_template("mytool", None);
        assert!(
            out.contains(r#"bsd: { pkg: "mytool" }"#),
            "BSD pkg placeholder not substituted to tool name: {}",
            out
        );
    }

    #[test]
    fn render_tool_template_some_bin_differs_from_none() {
        // When `bin` is provided, the `command:` field should reflect
        // it (not the tool name). When `None`, the bin defaults to the
        // tool name.
        let with_bin = render_tool_template("mytool", Some("mt"));
        assert!(
            with_bin.contains(r#"command: "mt""#),
            "explicit bin should be used as command: {}",
            with_bin
        );
        let without_bin = render_tool_template("mytool", None);
        assert!(
            without_bin.contains(r#"command: "mytool""#),
            "default bin should equal tool name: {}",
            without_bin
        );
        assert_ne!(
            with_bin, without_bin,
            "Some(bin) and None must produce different output"
        );
    }

    #[test]
    fn render_tool_template_upper_case_replaces_hyphens() {
        // `__TOOL_UPPER__` substitution: uppercase + hyphen→underscore
        // so the resulting Rust identifier is valid.
        let out = render_tool_template("docker-compose", None);
        assert!(
            out.contains("define_tool!(DOCKER_COMPOSE,"),
            "upper-with-underscore not applied: {}",
            out
        );
    }

    #[test]
    fn render_tool_template_winget_id_is_publisher_dot_name() {
        let out = render_tool_template("mytool", None);
        assert!(
            out.contains(r#"winget: "Publisher.mytool""#),
            "winget id should be Publisher.<tool>: {}",
            out
        );
    }

    // ----- iter_tool_names cache -----

    #[test]
    fn iter_tool_names_is_sorted() {
        let names: Vec<&str> = iter_tool_names().collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "iter_tool_names must yield sorted output");
    }

    #[test]
    fn iter_tool_names_includes_manual_tools() {
        let names: Vec<&str> = iter_tool_names().collect();
        // MANUAL_TOOLS are nvm/rust/brew — must appear regardless of
        // the inventory's `define_tool!` collection.
        for required in &["nvm", "rust", "brew"] {
            assert!(
                names.contains(required),
                "{} missing from iter_tool_names: {:?}",
                required,
                names
            );
        }
    }

    #[test]
    fn iter_tool_names_has_no_duplicates() {
        let names: Vec<&str> = iter_tool_names().collect();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            names.len(),
            sorted.len(),
            "iter_tool_names must dedup on cache build"
        );
    }

    #[test]
    fn iter_tool_names_two_calls_yield_same_data() {
        let a: Vec<&str> = iter_tool_names().collect();
        let b: Vec<&str> = iter_tool_names().collect();
        assert_eq!(a, b, "cache must yield identical content on repeat calls");
    }

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
        bsd: None,
        custom_install: None,
        default_hook: None,
        depends_on: None,
        depends_on_one_of: None,
        category: None,
    };

    // Test ToolSpec with a default hook
    static TEST_TOOL_WITH_HOOK: ToolSpec = ToolSpec {
        name: "test_hooked",
        command: "test_hooked_cmd",
        macos: Some(MacOsInstall::brew("test")),
        linux: Some(LinuxInstall::uniform("test")),
        windows: None,
        bsd: None,
        custom_install: None,
        default_hook: Some(DefaultHook::new(
            "Configure test tool",
            "echo 'test hook executed'",
        )),
        depends_on: None,
        depends_on_one_of: None,
        category: None,
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
            bsd: None,
            custom_install: None,
            default_hook: None,
            depends_on: None,
            depends_on_one_of: None,
            category: None,
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
            bsd: None,
            custom_install: Some(|_| Ok(())),
            default_hook: None,
            depends_on: None,
            depends_on_one_of: None,
            category: None,
        };
        let entry = ToolIndexEntry::from(&custom_tool);
        assert!(entry.custom_install.has_custom_installer);
    }

    #[test]
    fn test_generate_tool_index_has_tools() {
        let index = generate_tool_index();
        // Should have at least the 3 manual tools (nvm, rust, brew)
        assert!(
            index.count >= 3,
            "Expected at least 3 tools, got {}",
            index.count
        );
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
            bsd: None,
            custom_install: None,
            #[cfg(target_os = "macos")]
            default_hook: Some(DefaultHook::for_platform(
                "macOS hook",
                "brew info",
                "macos",
            )),
            #[cfg(target_os = "linux")]
            default_hook: Some(DefaultHook::for_platform("Linux hook", "apt info", "linux")),
            #[cfg(target_os = "windows")]
            default_hook: Some(DefaultHook::for_platform(
                "Windows hook",
                "winget info",
                "windows",
            )),
            depends_on: None,
            depends_on_one_of: None,
            category: None,
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

    // ========================================================================
    // Dependency Ordering Tests
    // ========================================================================

    #[test]
    fn test_get_tool_dependencies_no_deps() {
        // Tools without dependencies should return empty slice
        let deps = get_tool_dependencies("nonexistent");
        assert!(deps.is_empty());
    }

    #[test]
    fn test_tool_has_dependencies() {
        // Unknown tools have no dependencies
        assert!(!tool_has_dependencies("nonexistent"));
        // Most standard tools have no dependencies
        assert!(!tool_has_dependencies("git"));
    }

    #[test]
    fn test_order_tools_no_dependencies() {
        // Tools without dependencies should maintain relative order
        let tools = vec![("git", "2.0"), ("jq", "1.6"), ("curl", "7.0")];
        let ordered = order_tools_by_dependencies(tools.into_iter());
        assert_eq!(ordered.len(), 3);
        // Names should be lowercase
        assert!(ordered.iter().any(|(n, _)| n == "git"));
        assert!(ordered.iter().any(|(n, _)| n == "jq"));
        assert!(ordered.iter().any(|(n, _)| n == "curl"));
    }

    #[test]
    fn test_order_tools_empty_input() {
        let tools: Vec<(&str, &str)> = vec![];
        let ordered = order_tools_by_dependencies(tools.into_iter());
        assert!(ordered.is_empty());
    }

    #[test]
    fn test_order_tools_single_tool() {
        let tools = vec![("git", "2.0")];
        let ordered = order_tools_by_dependencies(tools.into_iter());
        assert_eq!(ordered.len(), 1);
        assert_eq!(ordered[0].0, "git");
        assert_eq!(ordered[0].1, "2.0");
    }

    #[test]
    fn test_order_tools_preserves_versions() {
        let tools = vec![("git", "2.40.0"), ("jq", "1.7")];
        let ordered = order_tools_by_dependencies(tools.into_iter());

        let git_entry = ordered.iter().find(|(n, _)| n == "git").unwrap();
        let jq_entry = ordered.iter().find(|(n, _)| n == "jq").unwrap();

        assert_eq!(git_entry.1, "2.40.0");
        assert_eq!(jq_entry.1, "1.7");
    }

    #[test]
    fn test_order_tools_case_insensitive() {
        let tools = vec![("GIT", "2.0"), ("JQ", "1.6")];
        let ordered = order_tools_by_dependencies(tools.into_iter());

        // All names should be lowercase in output
        for (name, _) in &ordered {
            assert_eq!(name, &name.to_lowercase());
        }
    }

    // ========================================================================
    // Flexible Dependency Tests
    // ========================================================================

    #[test]
    fn test_get_tool_flexible_dependencies_no_deps() {
        // Tools without flexible dependencies should return empty slice
        let deps = get_tool_flexible_dependencies("nonexistent");
        assert!(deps.is_empty());

        // git has no flexible dependencies
        let deps = get_tool_flexible_dependencies("git");
        assert!(deps.is_empty());
    }

    #[test]
    fn test_tool_has_flexible_dependencies() {
        // Unknown tools have no flexible dependencies
        assert!(!tool_has_flexible_dependencies("nonexistent"));
        // Most standard tools have no flexible dependencies
        assert!(!tool_has_flexible_dependencies("git"));
    }

    #[test]
    fn test_tool_has_any_dependencies() {
        // Unknown tools have no dependencies
        assert!(!tool_has_any_dependencies("nonexistent"));
        // Standard tools without deps
        assert!(!tool_has_any_dependencies("git"));
    }

    #[test]
    fn test_dependency_check_result_methods() {
        // Satisfied
        let satisfied = DependencyCheckResult::Satisfied;
        assert!(satisfied.is_satisfied());
        assert!(!satisfied.has_missing_required());
        assert!(!satisfied.has_missing_flexible());

        // WillInstallFlexible
        let will_install = DependencyCheckResult::WillInstallFlexible("docker".to_string());
        assert!(will_install.is_satisfied()); // This is still "satisfied" from ordering perspective
        assert!(!will_install.has_missing_required());
        assert!(!will_install.has_missing_flexible());

        // MissingRequired
        let missing_req = DependencyCheckResult::MissingRequired(vec!["docker".to_string()]);
        assert!(!missing_req.is_satisfied());
        assert!(missing_req.has_missing_required());
        assert!(!missing_req.has_missing_flexible());

        // MissingFlexible
        let missing_flex = DependencyCheckResult::MissingFlexible {
            needed: "container runtime",
            options: vec!["docker".to_string(), "podman".to_string()],
            suggestion: Some("docker".to_string()),
        };
        assert!(!missing_flex.is_satisfied());
        assert!(!missing_flex.has_missing_required());
        assert!(missing_flex.has_missing_flexible());
    }

    #[test]
    fn test_check_tool_dependencies_unknown_tool() {
        use std::collections::HashSet;
        let config_tools = HashSet::new();
        let installed_tools = HashSet::new();

        let result =
            check_tool_dependencies("nonexistent_tool_xyz", &config_tools, &installed_tools);
        assert_eq!(result, DependencyCheckResult::Satisfied);
    }

    #[test]
    fn test_check_tool_dependencies_no_deps() {
        use std::collections::HashSet;
        let config_tools = HashSet::new();
        let installed_tools = HashSet::new();

        // git has no dependencies
        let result = check_tool_dependencies("git", &config_tools, &installed_tools);
        assert_eq!(result, DependencyCheckResult::Satisfied);
    }

    #[test]
    fn test_check_tool_dependencies_strict_in_config() {
        use std::collections::HashSet;
        let mut config_tools = HashSet::new();
        config_tools.insert("docker".to_string());
        let installed_tools = HashSet::new();

        // lazydocker depends on docker - docker is in config so should be satisfied
        let result = check_tool_dependencies("lazydocker", &config_tools, &installed_tools);
        assert!(result.is_satisfied());
    }

    #[test]
    fn test_check_tool_dependencies_strict_installed() {
        use std::collections::HashSet;
        let config_tools = HashSet::new();
        let mut installed_tools = HashSet::new();
        installed_tools.insert("docker".to_string());

        // lazydocker depends on docker - docker is installed so should be satisfied
        let result = check_tool_dependencies("lazydocker", &config_tools, &installed_tools);
        assert!(result.is_satisfied());
    }

    #[test]
    fn test_check_tool_dependencies_strict_missing() {
        use std::collections::HashSet;
        let config_tools = HashSet::new();
        let installed_tools = HashSet::new();

        // lazydocker depends on docker - docker is missing
        let result = check_tool_dependencies("lazydocker", &config_tools, &installed_tools);
        assert!(result.has_missing_required());
        if let DependencyCheckResult::MissingRequired(missing) = result {
            assert!(missing.contains(&"docker".to_string()));
        } else {
            panic!("Expected MissingRequired");
        }
    }
}
