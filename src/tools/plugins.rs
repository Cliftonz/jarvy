//! User-defined tool plugins from ~/.jarvy/tools.d/
//!
//! Loads TOML tool definitions from the user's plugin directory and registers
//! them alongside built-in tools. User tools can override built-in tools.
//!
//! ## Plugin Format
//!
//! Each `.toml` file in `~/.jarvy/tools.d/` defines one tool:
//!
//! ```toml
//! name = "my-tool"
//! command = "my-tool"
//!
//! [macos]
//! brew = "my-tool"
//!
//! [linux]
//! uniform = "my-tool"
//!
//! [windows]
//! winget = "Publisher.MyTool"
//! ```
//!
//! ## Security
//!
//! - The `~/.jarvy/tools.d/` directory and each `.toml` file MUST be owned by
//!   the current user and not writable by group/other (mode `& 0o022 == 0`).
//!   Plugins violating this are skipped with a warning to prevent local
//!   privilege escalation via dropped TOML files.
//! - Package names are validated against `[A-Za-z0-9._/+@:-]+` before being
//!   passed to `brew`/`apt-get`/`winget`/`choco` to prevent argument injection.

use crate::tools::common::{InstallError, Os, current_os, has, run};
use crate::tools::registry::register_tool;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

/// Parsed plugin tool definition
#[derive(Debug, Deserialize, Clone)]
pub struct PluginTool {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub macos: Option<PluginPlatform>,
    #[serde(default)]
    pub linux: Option<PluginPlatform>,
    #[serde(default)]
    pub windows: Option<PluginPlatform>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)] // Fields used for deserialization
pub struct PluginPlatform {
    pub brew: Option<String>,
    pub cask: Option<String>,
    pub uniform: Option<String>,
    pub apt: Option<String>,
    pub dnf: Option<String>,
    pub pacman: Option<String>,
    pub winget: Option<String>,
    pub choco: Option<String>,
}

/// Map of plugin name (lowercased) -> definition. Populated by `load_user_tools`.
static PLUGIN_REGISTRY: OnceLock<RwLock<HashMap<String, PluginTool>>> = OnceLock::new();

fn registry() -> &'static RwLock<HashMap<String, PluginTool>> {
    PLUGIN_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Get the plugin directory path
fn plugin_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".jarvy").join("tools.d"))
}

/// Returns true when the path's permissions are safe to load from on Unix:
/// not group-writable, not other-writable. On non-Unix platforms always true.
#[cfg(unix)]
fn is_path_safe_to_load(p: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let Ok(meta) = std::fs::metadata(p) else {
        return false;
    };
    let mode = meta.mode();
    if mode & 0o022 != 0 {
        return false;
    }
    // Best-effort owner check: if uid is reachable, ensure it matches.
    let current_uid = libc_getuid();
    if let Some(uid) = current_uid {
        if meta.uid() != uid {
            return false;
        }
    }
    true
}

#[cfg(not(unix))]
fn is_path_safe_to_load(_p: &Path) -> bool {
    true
}

#[cfg(unix)]
#[allow(unsafe_code)]
unsafe extern "C" {
    fn getuid() -> u32;
}

#[cfg(unix)]
fn libc_getuid() -> Option<u32> {
    // SAFETY: getuid() is async-signal-safe and always succeeds; the FFI
    // signature matches POSIX `uid_t` width on every platform jarvy supports.
    #[allow(unsafe_code)]
    Some(unsafe { getuid() })
}

/// Validate a package-manager argument: only `[A-Za-z0-9._/+@:-]+` permitted.
/// Whitespace, shell metacharacters, and control characters are rejected.
pub(crate) fn is_valid_package_name(s: &str) -> bool {
    !s.is_empty()
        && s.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '+' | '@' | ':' | '-')
        })
}

/// Load all plugin tools from `~/.jarvy/tools.d/*.toml`.
///
/// Returns the number of plugin tools registered. Skips files that:
/// - Have wrong extension or fail to parse
/// - Live in a group/other-writable directory or are themselves writable by
///   anyone but the owner (Unix only)
/// - Carry an invalid package name in any platform section
///
/// Idempotent: subsequent calls short-circuit using the previously-built
/// registry rather than re-reading and re-parsing every TOML file.
pub fn load_user_tools() -> usize {
    {
        let map = registry().read().expect("plugin registry rwlock poisoned");
        if !map.is_empty() {
            return map.len();
        }
    }

    let Some(dir) = plugin_dir() else {
        return 0;
    };

    if !dir.exists() {
        return 0;
    }

    if !is_path_safe_to_load(&dir) {
        tracing::warn!(
            event = "plugins.tools_d_unsafe_perms",
            path = %crate::network::redact_home(&dir.display().to_string()),
            "tools.d directory has insecure permissions; skipping plugin load"
        );
        return 0;
    }

    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                event = "plugins.read_dir_failed",
                path = %crate::network::redact_home(&dir.display().to_string()),
                error = %e,
            );
            return 0;
        }
    };

    let mut accepted: HashMap<String, PluginTool> = HashMap::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }

        if !is_path_safe_to_load(&path) {
            tracing::warn!(
                event = "plugins.file_unsafe_perms",
                path = %crate::network::redact_home(&path.display().to_string()),
                "plugin file has insecure permissions; skipping"
            );
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    event = "plugins.read_failed",
                    path = %crate::network::redact_home(&path.display().to_string()),
                    error = %e,
                );
                continue;
            }
        };

        let tool: PluginTool = match toml::from_str(&content) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(
                    event = "plugins.parse_failed",
                    path = %crate::network::redact_home(&path.display().to_string()),
                    error = %e,
                );
                continue;
            }
        };

        if !is_valid_package_name(&tool.name) || !is_valid_package_name(&tool.command) {
            tracing::warn!(
                event = "plugins.invalid_identifier",
                name = %tool.name,
                command = %tool.command,
                "plugin name/command contains disallowed characters; skipping"
            );
            continue;
        }

        if !validate_platforms(&tool) {
            tracing::warn!(
                event = "plugins.invalid_package_name",
                name = %tool.name,
                "plugin package strings contain disallowed characters; skipping"
            );
            continue;
        }

        let key = tool.name.to_ascii_lowercase();
        tracing::info!(
            event = "plugins.loaded",
            name = %tool.name,
            path = %crate::network::redact_home(&path.display().to_string()),
        );
        accepted.insert(key, tool);
    }

    let count = accepted.len();

    // Insert into the plugin registry first so dispatch can find them by name.
    {
        let mut map = registry().write().expect("plugin registry rwlock poisoned");
        for (key, tool) in accepted.iter() {
            map.insert(key.clone(), tool.clone());
        }
    }

    // Register a stub handler in the main registry so existing
    // `tools::add(name, version)` lookups find the plugin. The handler
    // never executes — `tools::add` consults the plugin registry first.
    for tool in accepted.values() {
        let _ = register_tool(&tool.name, plugin_install_handler_unreachable);
    }

    tracing::info!(event = "plugins.registered", count = count);
    count
}

/// Stub handler that should never be invoked because `tools::add` checks the
/// plugin registry first. Returns `Unsupported` if it ever is reached.
fn plugin_install_handler_unreachable(_version: &str) -> Result<(), InstallError> {
    Err(InstallError::Parse(
        "plugin handler invoked without name dispatch (bug)",
    ))
}

fn validate_platforms(tool: &PluginTool) -> bool {
    let check = |opt: &Option<String>| opt.as_deref().is_none_or(is_valid_package_name);
    let check_platform = |p: &PluginPlatform| {
        check(&p.brew)
            && check(&p.cask)
            && check(&p.uniform)
            && check(&p.apt)
            && check(&p.dnf)
            && check(&p.pacman)
            && check(&p.winget)
            && check(&p.choco)
    };
    tool.macos.as_ref().is_none_or(check_platform)
        && tool.linux.as_ref().is_none_or(check_platform)
        && tool.windows.as_ref().is_none_or(check_platform)
}

/// Look up a plugin tool by name (case-insensitive).
pub fn get_plugin(name: &str) -> Option<PluginTool> {
    let key = name.to_ascii_lowercase();
    let map = registry().read().expect("plugin registry rwlock poisoned");
    map.get(&key).cloned()
}

/// Install the plugin tool with the given name. Returns `Ok(true)` if a
/// plugin handled the request, `Ok(false)` if no plugin is registered under
/// that name (caller should fall through to built-in dispatch).
pub fn install_by_name(name: &str, _version: &str) -> Result<bool, InstallError> {
    let Some(tool) = get_plugin(name) else {
        return Ok(false);
    };

    let os = current_os();

    // Idempotency: if the command is already installed, skip.
    if has(&tool.command) {
        tracing::info!(
            event = "plugin.install.skip_already_installed",
            tool = %tool.name,
            command = %tool.command,
        );
        return Ok(true);
    }

    let result = match os {
        Os::Macos => install_macos(&tool),
        Os::Linux => install_linux(&tool),
        Os::Windows => install_windows(&tool),
        Os::Bsd => Err(InstallError::Unsupported),
    };

    match &result {
        Ok(()) => tracing::info!(
            event = "plugin.install.success",
            tool = %tool.name,
        ),
        Err(e) => tracing::warn!(
            event = "plugin.install.failed",
            tool = %tool.name,
            error = %e,
        ),
    }
    result.map(|()| true)
}

fn install_macos(tool: &PluginTool) -> Result<(), InstallError> {
    let Some(platform) = tool.macos.as_ref() else {
        return Err(InstallError::Unsupported);
    };
    if let Some(brew) = platform.brew.as_deref() {
        return run("brew", &["install", brew]).map(|_| ());
    }
    if let Some(cask) = platform.cask.as_deref() {
        return run("brew", &["install", "--cask", cask]).map(|_| ());
    }
    Err(InstallError::Unsupported)
}

fn install_linux(tool: &PluginTool) -> Result<(), InstallError> {
    let Some(platform) = tool.linux.as_ref() else {
        return Err(InstallError::Unsupported);
    };
    if let Some(uniform) = platform.uniform.as_deref() {
        if has("brew") {
            return run("brew", &["install", uniform]).map(|_| ());
        }
        if has("apt-get") {
            return run("apt-get", &["install", "-y", uniform]).map(|_| ());
        }
        if has("dnf") {
            return run("dnf", &["install", "-y", uniform]).map(|_| ());
        }
        if has("pacman") {
            return run("pacman", &["-S", "--noconfirm", uniform]).map(|_| ());
        }
    }
    if let Some(apt) = platform.apt.as_deref() {
        if has("apt-get") {
            return run("apt-get", &["install", "-y", apt]).map(|_| ());
        }
    }
    if let Some(dnf) = platform.dnf.as_deref() {
        if has("dnf") {
            return run("dnf", &["install", "-y", dnf]).map(|_| ());
        }
    }
    if let Some(pacman) = platform.pacman.as_deref() {
        if has("pacman") {
            return run("pacman", &["-S", "--noconfirm", pacman]).map(|_| ());
        }
    }
    if let Some(brew) = platform.brew.as_deref() {
        if has("brew") {
            return run("brew", &["install", brew]).map(|_| ());
        }
    }
    Err(InstallError::Unsupported)
}

fn install_windows(tool: &PluginTool) -> Result<(), InstallError> {
    let Some(platform) = tool.windows.as_ref() else {
        return Err(InstallError::Unsupported);
    };
    if let Some(winget) = platform.winget.as_deref() {
        return run("winget", &["install", "-e", "--id", winget]).map(|_| ());
    }
    if let Some(choco) = platform.choco.as_deref() {
        return run("choco", &["install", "-y", choco]).map(|_| ());
    }
    Err(InstallError::Unsupported)
}

/// List all loaded plugin tool names
#[allow(dead_code)] // Public API for future use
pub fn loaded_plugin_names() -> Vec<String> {
    let map = registry().read().expect("plugin registry rwlock poisoned");
    let mut names: Vec<String> = map.values().map(|t| t.name.clone()).collect();
    names.sort();
    names
}

/// Test helper: insert a plugin definition directly into the registry.
/// Prefixed with `_test_` and `#[doc(hidden)]` to discourage production use.
#[doc(hidden)]
pub fn _test_register(tool: PluginTool) {
    let key = tool.name.to_ascii_lowercase();
    registry()
        .write()
        .expect("plugin registry rwlock poisoned")
        .insert(key, tool);
}

/// Test helper: clear the plugin registry between tests.
#[doc(hidden)]
pub fn _test_clear() {
    registry()
        .write()
        .expect("plugin registry rwlock poisoned")
        .clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_package_name_allowlist() {
        assert!(is_valid_package_name("git"));
        assert!(is_valid_package_name("Microsoft.VisualStudioCode"));
        assert!(is_valid_package_name("user/tap/payload"));
        assert!(is_valid_package_name("docker-compose"));
        assert!(is_valid_package_name("python3.12"));

        assert!(!is_valid_package_name(""));
        assert!(!is_valid_package_name("git;rm -rf /"));
        assert!(!is_valid_package_name("git rm"));
        assert!(!is_valid_package_name("$(curl evil)"));
        assert!(!is_valid_package_name("git`whoami`"));
        assert!(!is_valid_package_name("git|nc"));
        assert!(!is_valid_package_name("git\nrm"));
    }

    #[test]
    fn install_by_name_returns_false_for_unknown() {
        _test_clear();
        let result = install_by_name("definitely-not-a-plugin", "latest");
        assert!(matches!(result, Ok(false)));
    }

    #[test]
    fn validate_platforms_rejects_injection_in_package_field() {
        let bad = PluginTool {
            name: "x".into(),
            command: "x".into(),
            macos: Some(PluginPlatform {
                brew: Some("legit; rm -rf /".into()),
                cask: None,
                uniform: None,
                apt: None,
                dnf: None,
                pacman: None,
                winget: None,
                choco: None,
            }),
            linux: None,
            windows: None,
        };
        assert!(!validate_platforms(&bad));

        let good = PluginTool {
            name: "x".into(),
            command: "x".into(),
            macos: Some(PluginPlatform {
                brew: Some("legit-package".into()),
                cask: None,
                uniform: None,
                apt: None,
                dnf: None,
                pacman: None,
                winget: None,
                choco: None,
            }),
            linux: None,
            windows: None,
        };
        assert!(validate_platforms(&good));
    }

    #[test]
    fn registry_lookup_returns_inserted_plugin() {
        _test_clear();
        _test_register(PluginTool {
            name: "TeStPlUg".into(),
            command: "testplug".into(),
            macos: None,
            linux: None,
            windows: None,
        });
        let found = get_plugin("testplug").expect("plugin should be present case-insensitive");
        assert_eq!(found.name, "TeStPlUg");
        assert_eq!(found.command, "testplug");
    }
}
