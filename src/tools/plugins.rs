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
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

/// Parsed plugin tool definition. `Serialize` is enabled so
/// `registry_remote::sync` can write the parsed remote-tool set to a
/// single JSON index after a successful sync — the plugin loader then
/// reads ONE file at CLI startup instead of stat+open+parse N TOMLs.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PluginTool {
    pub name: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macos: Option<PluginPlatform>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linux: Option<PluginPlatform>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub windows: Option<PluginPlatform>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[allow(dead_code)] // Fields used for deserialization
pub struct PluginPlatform {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brew: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cask: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uniform: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dnf: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pacman: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub winget: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choco: Option<String>,
}

/// Map of plugin name (lowercased) -> definition. Populated by `load_user_tools`.
static PLUGIN_REGISTRY: OnceLock<RwLock<HashMap<String, PluginTool>>> = OnceLock::new();

fn registry() -> &'static RwLock<HashMap<String, PluginTool>> {
    PLUGIN_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Get the plugin directory path. Routed through `crate::paths` so a
/// `JARVY_HOME` override / future XDG migration is honored.
fn plugin_dir() -> Option<PathBuf> {
    crate::paths::plugins_dir().ok()
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

    let mut accepted: HashMap<String, PluginTool> = HashMap::new();

    // 1. User-authored plugins at ~/.jarvy/tools.d/*.toml.
    if let Some(user_dir) = plugin_dir() {
        load_tools_from_dir(&user_dir, &mut accepted);
    }

    // 2. Tools synced from a remote registry via `jarvy registry sync`.
    //    Cached under ~/.jarvy/tools.d/.remote/tools/. Try the parsed
    //    JSON index first — one read instead of N stat+open+TOML-parse.
    //    Falls back to walking the dir when the index is missing,
    //    perms-rejected, or stale relative to meta.json. Either way the
    //    same security gates (perms, name validation, platform
    //    validation) apply.
    if let Some(remote_tools) = try_load_remote_index() {
        for tool in remote_tools {
            let key = tool.name.to_ascii_lowercase();
            accepted.insert(key, tool);
        }
    } else if let Ok(remote_dir) = crate::paths::registry_remote_cache_dir() {
        let tools_subdir = remote_dir.join("tools");
        if tools_subdir.exists() {
            load_tools_from_dir(&tools_subdir, &mut accepted);
        }
    }

    let count = accepted.len();

    // Register a stub handler in the main registry first so existing
    // `tools::add(name, version)` lookups find the plugin. The handler
    // never executes — `tools::add` consults the plugin registry first.
    for tool in accepted.values() {
        let _ = register_tool(&tool.name, plugin_install_handler_unreachable);
    }

    // Then drain into the plugin registry. Drain (not clone) avoids
    // duplicating every key + PluginTool when populating the lock —
    // `accepted` is about to drop anyway.
    {
        let mut map = registry().write().expect("plugin registry rwlock poisoned");
        for (key, tool) in accepted.drain() {
            map.insert(key, tool);
        }
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

/// Cached parsed-tools snapshot written by `registry_remote::sync` at the
/// end of a successful sync. The plugin loader prefers this single JSON
/// read over walking `~/.jarvy/tools.d/.remote/tools/` + per-file open +
/// TOML parse on every CLI startup.
///
/// `synced_at_unix` mirrors `meta.json::last_synced_at_unix` and is the
/// invalidation key — if the loader sees the values disagree (someone
/// hand-edited the cache between syncs) the index is treated as stale
/// and the walk-and-parse fallback runs.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RemoteIndex {
    pub synced_at_unix: u64,
    pub tools: Vec<PluginTool>,
}

/// Walk `~/.jarvy/tools.d/.remote/tools/` once, parse every TOML, and
/// write the resulting `PluginTool` set as a single JSON blob at
/// `~/.jarvy/tools.d/.remote/index.json`. Called by
/// `registry_remote::sync::run_sync_with_config` after the staging-swap
/// succeeds; the next CLI startup will then read one file instead of N.
///
/// Failures here are non-fatal — they only fall back to the walk-and-
/// parse path at next load. We log a warning so a fleet operator can
/// notice but the sync still reports success because the actual TOMLs
/// are on disk.
pub fn build_remote_index(synced_at_unix: u64, tools: Vec<PluginTool>) -> std::io::Result<()> {
    let Ok(remote_root) = crate::paths::registry_remote_cache_dir() else {
        return Ok(());
    };

    let accepted_count = tools.len();
    let index = RemoteIndex {
        synced_at_unix,
        tools,
    };
    let index_path = remote_root.join("index.json");
    let payload = serde_json::to_vec(&index).map_err(std::io::Error::other)?;

    // Atomic-ish write via tmp + rename + stat-after-chmod (matches the
    // cache layer's hardening: silent chmod failures on NFS/exFAT used
    // to leave index.json world-readable).
    let tmp = index_path.with_extension("json.tmp");
    let _ = std::fs::remove_file(&tmp);
    std::fs::write(&tmp, &payload)?;
    std::fs::rename(&tmp, &index_path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&index_path, std::fs::Permissions::from_mode(0o600))?;
        let mode = std::fs::metadata(&index_path)?.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            crate::observability::telemetry_gate::emit(|| {
                tracing::warn!(
                    event = "registry.cache.index_perms_unsafe",
                    mode = format!("{:#o}", mode),
                );
            });
            // Best effort: delete the file rather than leave it world-readable.
            let _ = std::fs::remove_file(&index_path);
            return Err(std::io::Error::other(format!(
                "index.json mode {mode:#o} grants group/other access; refusing to leave it on disk"
            )));
        }
    }

    crate::observability::telemetry_gate::emit(|| {
        tracing::info!(
            event = "registry.cache.index_built",
            accepted_count = accepted_count,
        );
    });
    Ok(())
}

/// Try to load the remote-tool set from the cached JSON index.
///
/// Returns `Some(tools)` only if:
/// 1. `index.json` exists with safe perms
/// 2. `meta.json` exists with safe perms
/// 3. Both parse and their `synced_at_unix`/`last_synced_at_unix`
///    fields agree
///
/// Any other shape (missing file, mismatched timestamp, parse error)
/// returns `None` and the caller falls back to walking the tools/
/// directory. Avoiding hard errors keeps the loader resilient against
/// a partially-written cache from an interrupted older Jarvy.
fn try_load_remote_index() -> Option<Vec<PluginTool>> {
    let remote_root = crate::paths::registry_remote_cache_dir().ok()?;
    let index_path = remote_root.join("index.json");
    let meta_path = remote_root.join("meta.json");
    if !is_path_safe_to_load(&index_path) {
        emit_index_miss("unsafe_perms");
        return None;
    }
    if !is_path_safe_to_load(&meta_path) {
        emit_index_miss("unsafe_perms");
        return None;
    }

    let Ok(index_bytes) = std::fs::read(&index_path) else {
        emit_index_miss("no_index");
        return None;
    };
    let Ok(meta_bytes) = std::fs::read(&meta_path) else {
        emit_index_miss("no_meta");
        return None;
    };
    let Ok(index) = serde_json::from_slice::<RemoteIndex>(&index_bytes) else {
        emit_index_miss("parse_failed");
        return None;
    };
    let Ok(meta) = serde_json::from_slice::<serde_json::Value>(&meta_bytes) else {
        emit_index_miss("parse_failed");
        return None;
    };
    let meta_synced = meta.get("last_synced_at_unix").and_then(|v| v.as_u64())?;
    if meta_synced != index.synced_at_unix {
        emit_index_miss("stale_timestamp");
        return None;
    }

    // Per-tool security gates: the index was BUILT by trusted sync code,
    // but anything could have hand-edited index.json between sync and
    // load. Re-run the same validation `load_tools_from_dir` applies to
    // the walk fallback. A single bad entry rejects the whole index —
    // caller falls back to the walk, which surfaces the bad TOML with
    // its own warn event.
    for tool in &index.tools {
        if !is_valid_package_name(&tool.name) || !is_valid_package_name(&tool.command) {
            emit_index_miss("invalid_identifier");
            return None;
        }
        if !validate_platforms(tool) {
            emit_index_miss("invalid_platform_package");
            return None;
        }
    }

    crate::observability::telemetry_gate::emit(|| {
        tracing::debug!(
            event = "registry.cache.index_hit",
            tools_count = index.tools.len(),
            synced_at_unix = index.synced_at_unix,
        );
    });
    Some(index.tools)
}

fn emit_index_miss(reason: &'static str) {
    crate::observability::telemetry_gate::emit(|| {
        tracing::debug!(event = "registry.cache.index_miss", reason = reason);
    });
}

/// Walk one directory of TOML plugin files and insert any that pass
/// validation into `accepted`. Both the user `tools.d/` dir and the
/// remote-registry cache `tools.d/.remote/tools/` dir flow through here
/// so the security gates (dir perms, file perms, name validation,
/// platform validation) are applied uniformly. Conflicts: a later entry
/// for the same name wins — by call order in `load_user_tools`,
/// remote-synced tools override user-authored ones if they share a name.
fn load_tools_from_dir(dir: &std::path::Path, accepted: &mut HashMap<String, PluginTool>) {
    if !is_path_safe_to_load(dir) {
        tracing::warn!(
            event = "plugins.tools_d_unsafe_perms",
            path = %crate::network::redact_home(&dir.display().to_string()),
            "plugin directory has insecure permissions; skipping load"
        );
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                event = "plugins.read_dir_failed",
                path = %crate::network::redact_home(&dir.display().to_string()),
                error = %e,
            );
            return;
        }
    };

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

    fn write_plugin(dir: &Path, name: &str, command: &str) {
        std::fs::write(
            dir.join(format!("{name}.toml")),
            format!(
                r#"name = "{name}"
command = "{command}"
"#
            ),
        )
        .expect("write plugin");
    }

    /// Item 5 part 1 — `load_tools_from_dir` walks a single dir and
    /// registers everything that passes validation.
    #[test]
    fn load_tools_from_dir_registers_each_toml() {
        let tmp = tempfile::tempdir().expect("tempdir");
        write_plugin(tmp.path(), "alpha", "alpha");
        write_plugin(tmp.path(), "beta", "beta");

        let mut accepted = HashMap::new();
        load_tools_from_dir(tmp.path(), &mut accepted);
        assert_eq!(accepted.len(), 2);
        assert!(accepted.contains_key("alpha"));
        assert!(accepted.contains_key("beta"));
    }

    /// Item 5 part 2 — collision precedence. The plugin loader walks
    /// user dir THEN remote cache; a later insert with the same key
    /// shadows the earlier one (remote wins over user-authored). Pin
    /// this behavior so a future refactor that reorders the walks
    /// fails this test.
    #[test]
    fn load_tools_from_dir_later_call_overrides_earlier() {
        let user_dir = tempfile::tempdir().expect("user tempdir");
        let remote_dir = tempfile::tempdir().expect("remote tempdir");

        std::fs::write(
            user_dir.path().join("collide.toml"),
            r#"name = "collide"
command = "user-version"
"#,
        )
        .unwrap();
        std::fs::write(
            remote_dir.path().join("collide.toml"),
            r#"name = "collide"
command = "remote-version"
"#,
        )
        .unwrap();

        let mut accepted = HashMap::new();
        load_tools_from_dir(user_dir.path(), &mut accepted);
        load_tools_from_dir(remote_dir.path(), &mut accepted);
        // Remote-walked-second wins.
        assert_eq!(
            accepted.get("collide").map(|t| t.command.as_str()),
            Some("remote-version")
        );
    }

    /// Item 5 part 3 — invalid identifier silently skipped (NOT all-
    /// or-nothing).
    #[test]
    fn load_tools_from_dir_skips_invalid_name_keeps_valid() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // Valid one.
        write_plugin(tmp.path(), "good", "good");
        // Invalid: command has a shell metacharacter.
        std::fs::write(
            tmp.path().join("bad.toml"),
            r#"name = "bad"
command = "bad;rm -rf /"
"#,
        )
        .unwrap();

        let mut accepted = HashMap::new();
        load_tools_from_dir(tmp.path(), &mut accepted);
        assert!(accepted.contains_key("good"));
        assert!(!accepted.contains_key("bad"));
    }

    /// Item 5 part 4 — missing dir is a no-op, not a panic.
    #[test]
    fn load_tools_from_dir_missing_dir_is_noop() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let absent = tmp.path().join("does-not-exist");
        let mut accepted = HashMap::new();
        // Function exits early via is_path_safe_to_load → false (no
        // metadata) OR read_dir → err. Either way, accepted stays empty.
        load_tools_from_dir(&absent, &mut accepted);
        assert!(accepted.is_empty());
    }

    /// Item 1 (post-review fix) — `try_load_remote_index` MUST refuse
    /// to load tools whose `command` field contains shell metachars.
    /// Pre-fix the index path bypassed the per-tool validators that
    /// `load_tools_from_dir` ran; this pins the equivalence.
    #[test]
    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
    fn try_load_remote_index_rejects_invalid_command_field() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let prev = std::env::var_os("JARVY_HOME");
        // SAFETY: serial-test gate would help here but the test is
        // small; we restore on drop via the let-_ = guard pattern.
        unsafe {
            std::env::set_var("JARVY_HOME", tmp.path());
        }
        struct Guard(Option<std::ffi::OsString>);
        impl Drop for Guard {
            fn drop(&mut self) {
                // SAFETY: restore the previously-saved value.
                unsafe {
                    match &self.0 {
                        Some(v) => std::env::set_var("JARVY_HOME", v),
                        None => std::env::remove_var("JARVY_HOME"),
                    }
                }
            }
        }
        let _guard = Guard(prev);

        let remote = crate::paths::registry_remote_cache_dir().expect("cache dir resolves");
        std::fs::create_dir_all(&remote).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&remote, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let index = serde_json::json!({
            "synced_at_unix": 100u64,
            "tools": [{
                "name": "tool",
                "command": "evil;rm -rf ~",
                "macos": null, "linux": null, "windows": null
            }]
        });
        let meta = serde_json::json!({ "last_synced_at_unix": 100u64 });
        std::fs::write(remote.join("index.json"), index.to_string()).unwrap();
        std::fs::write(remote.join("meta.json"), meta.to_string()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                remote.join("index.json"),
                std::fs::Permissions::from_mode(0o600),
            )
            .unwrap();
            std::fs::set_permissions(
                remote.join("meta.json"),
                std::fs::Permissions::from_mode(0o600),
            )
            .unwrap();
        }

        let result = try_load_remote_index();
        assert!(
            result.is_none(),
            "index with shell-meta in command MUST fall back to walk, got {result:?}"
        );
    }

    /// Item 9 — try_load_remote_index returns None on each enumerated
    /// rejection condition.
    #[test]
    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
    fn try_load_remote_index_rejects_stale_timestamp() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let prev = std::env::var_os("JARVY_HOME");
        // SAFETY: see invalid_command_field test.
        unsafe {
            std::env::set_var("JARVY_HOME", tmp.path());
        }
        struct Guard(Option<std::ffi::OsString>);
        impl Drop for Guard {
            fn drop(&mut self) {
                // SAFETY: restore prior env.
                unsafe {
                    match &self.0 {
                        Some(v) => std::env::set_var("JARVY_HOME", v),
                        None => std::env::remove_var("JARVY_HOME"),
                    }
                }
            }
        }
        let _g = Guard(prev);

        let remote = crate::paths::registry_remote_cache_dir().expect("cache dir resolves");
        std::fs::create_dir_all(&remote).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&remote, std::fs::Permissions::from_mode(0o700)).unwrap();
        }

        // Index claims synced_at_unix=200, meta says 100. Mismatch → None.
        let index = serde_json::json!({ "synced_at_unix": 200u64, "tools": [] });
        let meta = serde_json::json!({ "last_synced_at_unix": 100u64 });
        std::fs::write(remote.join("index.json"), index.to_string()).unwrap();
        std::fs::write(remote.join("meta.json"), meta.to_string()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                remote.join("index.json"),
                std::fs::Permissions::from_mode(0o600),
            )
            .unwrap();
            std::fs::set_permissions(
                remote.join("meta.json"),
                std::fs::Permissions::from_mode(0o600),
            )
            .unwrap();
        }

        assert!(try_load_remote_index().is_none());
    }
}
