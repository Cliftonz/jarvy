//! Canonical resolver for `~/.jarvy/...` paths.
//!
//! Previously 22+ subsystems hand-rolled
//! `dirs::home_dir().map(|h| h.join(".jarvy").join("X"))` with four
//! different fallback policies and the literal `".jarvy"` in 60+ places.
//! Moving `~/.jarvy` (e.g. to `~/.local/share/jarvy` per XDG) used to
//! mean touching every site; with this module it's one constant.
//!
//! This is the natural seam for future XDG migration and for a
//! `JARVY_HOME` env override.

use std::path::{Component, Path, PathBuf};

/// Internal constant for the base directory name.
const JARVY_DIR: &str = ".jarvy";

/// Return the directory containing `file`, treating bare filenames
/// (empty parent component) as cwd. Centralizes the "project root
/// from a jarvy.toml path" pattern previously inlined across many
/// command handlers. Lives in `paths` (not `commands::setup_cmd`)
/// because library-side modules like `discover` also need it.
pub fn config_parent_dir(file: &str) -> PathBuf {
    Path::new(file)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Returned when `dirs::home_dir()` cannot be resolved (rare; running as
/// `nobody`, certain container images, etc.) OR when `JARVY_HOME` is
/// rejected as unsafe.
#[derive(Debug, thiserror::Error)]
#[error("cannot determine home directory")]
pub struct NoHomeDir;

/// `~/.jarvy/`. Honors `JARVY_HOME` if set so the user can override the
/// base location for tests and ad-hoc isolation.
///
/// `JARVY_HOME` is treated as a trust-boundary input: rejected if not
/// absolute or if it contains `..` traversal components. On Unix, if
/// the path already exists, ownership must match the current uid —
/// prevents `sudo -E jarvy ...` style attacks where a less-privileged
/// actor's env points a privileged jarvy run at e.g. `/etc` or
/// `/root/.ssh`. See PRD-053 security review F2.
pub fn jarvy_home() -> Result<PathBuf, NoHomeDir> {
    if let Ok(custom) = std::env::var("JARVY_HOME") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed);
            if !is_safe_jarvy_home(&p) {
                return Err(NoHomeDir);
            }
            return Ok(p);
        }
    }
    dirs::home_dir().map(|h| h.join(JARVY_DIR)).ok_or(NoHomeDir)
}

/// Validate a `JARVY_HOME` override: must be absolute, must not contain
/// `..` components, and (on Unix, if it exists) must be owned by the
/// current uid. Returns true if safe to use.
fn is_safe_jarvy_home(p: &std::path::Path) -> bool {
    if !p.is_absolute() {
        return false;
    }
    if p.components().any(|c| matches!(c, Component::ParentDir)) {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // Only check ownership if the path already exists; if jarvy
        // is the one creating it, the new dir will be owned by the
        // current uid by definition.
        if let Ok(meta) = std::fs::symlink_metadata(p)
            && meta.uid() != current_uid()
        {
            return false;
        }
    }
    true
}

#[cfg(unix)]
fn current_uid() -> u32 {
    // Avoid pulling in `libc` for one syscall. `getuid` is linked by
    // default via the platform libc on all Unix Rust targets we care
    // about; declaring it locally as an `unsafe extern "C"` block is
    // sufficient under Rust 2024 edition rules.
    #[allow(unsafe_code)]
    unsafe {
        unsafe extern "C" {
            fn getuid() -> u32;
        }
        getuid()
    }
}

/// `~/.jarvy/config.toml` — global user config.
pub fn config_toml() -> Result<PathBuf, NoHomeDir> {
    Ok(jarvy_home()?.join("config.toml"))
}

/// `~/.jarvy/logs/`.
pub fn logs_dir() -> Result<PathBuf, NoHomeDir> {
    Ok(jarvy_home()?.join("logs"))
}

/// `~/.jarvy/tickets/`.
pub fn tickets_dir() -> Result<PathBuf, NoHomeDir> {
    Ok(jarvy_home()?.join("tickets"))
}

/// `~/.jarvy/cache/configs/` — used by `remote::fetch_remote_config`.
pub fn remote_config_cache_dir() -> Result<PathBuf, NoHomeDir> {
    Ok(jarvy_home()?.join("cache").join("configs"))
}

/// `~/.jarvy/staging/` — pre-verify download landing zone for `update`.
pub fn staging_dir() -> Result<PathBuf, NoHomeDir> {
    Ok(jarvy_home()?.join("staging"))
}

/// `~/.jarvy/backup/` — pre-update binary copy for rollback.
pub fn backup_dir() -> Result<PathBuf, NoHomeDir> {
    Ok(jarvy_home()?.join("backup"))
}

/// `~/.jarvy/state/` — runtime state files (wizard-session tokens,
/// scratch flags). Distinct from `cache/` because contents here are
/// per-invocation and not safe to preserve across process restarts.
pub fn state_dir() -> Result<PathBuf, NoHomeDir> {
    Ok(jarvy_home()?.join("state"))
}

/// `~/.jarvy/tools.d/` — user plugin tool definitions.
pub fn plugins_dir() -> Result<PathBuf, NoHomeDir> {
    Ok(jarvy_home()?.join("tools.d"))
}

/// `~/.jarvy/tools.d/.remote/` — cache root for tools pulled from a remote
/// registry via `jarvy registry sync`. Sits inside `plugins_dir()` so the
/// existing plugin loader can walk both user-authored TOMLs and remote-
/// synced ones with the same security gates.
pub fn registry_remote_cache_dir() -> Result<PathBuf, NoHomeDir> {
    Ok(plugins_dir()?.join(".remote"))
}

/// `~/.jarvy/team-sources.toml` — team config source registry.
pub fn team_sources_toml() -> Result<PathBuf, NoHomeDir> {
    Ok(jarvy_home()?.join("team-sources.toml"))
}

/// `~/.jarvy/mcp-config.toml` — MCP allow/deny lists.
pub fn mcp_config_toml() -> Result<PathBuf, NoHomeDir> {
    Ok(jarvy_home()?.join("mcp-config.toml"))
}

/// `~/.jarvy/update-state.json` — last update-check timestamp.
pub fn update_state_json() -> Result<PathBuf, NoHomeDir> {
    Ok(jarvy_home()?.join("update-state.json"))
}

/// `~/.jarvy/install-method.json` — cached self-install-method detection.
pub fn install_method_json() -> Result<PathBuf, NoHomeDir> {
    Ok(jarvy_home()?.join("install-method.json"))
}

/// `~/.jarvy/rollback-info.json` — most-recent self-update rollback record.
pub fn rollback_info_json() -> Result<PathBuf, NoHomeDir> {
    Ok(jarvy_home()?.join("rollback-info.json"))
}

/// `~/.jarvy/ensure.stamp` — shell-init idempotency stamp.
pub fn ensure_stamp() -> Result<PathBuf, NoHomeDir> {
    Ok(jarvy_home()?.join("ensure.stamp"))
}

/// Project-local drift baseline state file: `<project>/.jarvy/state.json`.
pub fn state_json(project: &std::path::Path) -> PathBuf {
    project.join(JARVY_DIR).join("state.json")
}

/// Create `dir` if it doesn't exist; on Unix tighten its mode to 0o700 so
/// staging downloads / ticket bundles aren't readable by other users on a
/// shared host (security review F-15).
pub fn ensure_dir_0700(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn jarvy_home_honors_env_override() {
        // Serialized via #[serial(jarvy_home_env)] so concurrent tests
        // (e.g., ticket::bundler::tests::test_bundler_new which reads
        // tickets_dir()) don't observe our temporarily-set JARVY_HOME.
        //
        // Use an OS-appropriate absolute path under the platform tempdir.
        // `/tmp/jarvy-test-override` is NOT absolute on Windows (no drive
        // letter), so `is_safe_jarvy_home` rejected it and the test
        // hard-failed on every Windows tag-push CI run — silent tech debt
        // since v0.2.0-rc.1. `std::env::temp_dir()` resolves correctly
        // on every platform.
        let override_path = std::env::temp_dir().join("jarvy-test-override");
        let prev = std::env::var("JARVY_HOME").ok();
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("JARVY_HOME", &override_path);
        }
        let p = jarvy_home().unwrap();
        assert_eq!(p, override_path);

        // Cleanup.
        #[allow(unsafe_code)]
        unsafe {
            match prev {
                Some(v) => std::env::set_var("JARVY_HOME", v),
                None => std::env::remove_var("JARVY_HOME"),
            }
        }
    }

    #[test]
    #[serial_test::serial(jarvy_home_env)]
    fn derived_paths_share_jarvy_home() {
        // Serialized via #[serial(jarvy_home_env)] so the sibling override
        // test can't mutate JARVY_HOME mid-assertion.
        if std::env::var("JARVY_HOME").is_ok() {
            return;
        }
        let home = jarvy_home().unwrap();
        assert!(logs_dir().unwrap().starts_with(&home));
        assert!(tickets_dir().unwrap().starts_with(&home));
        assert!(remote_config_cache_dir().unwrap().starts_with(&home));
        assert!(staging_dir().unwrap().starts_with(&home));
        assert!(backup_dir().unwrap().starts_with(&home));
        assert!(config_toml().unwrap().starts_with(&home));
    }
}
