//! Shared filesystem helpers for AI hook provisioners.
//!
//! All on-disk writes go through an atomic tempfile-then-rename so a
//! partial settings file never lands. Tempfile names include the
//! process ID + nanoseconds so concurrent `jarvy setup` runs don't
//! stomp each other. Reads tolerate missing or empty files (returns an
//! empty object).
//!
//! Symlink safety: before writing, every helper refuses if the target
//! is already a symlink. `rename(2)` follows symlinks on the destination
//! on Linux / macOS — without this check, an attacker who can plant a
//! symlink at `~/.claude/settings.json` could redirect our write at
//! arbitrary files inside `$HOME`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};

use crate::ai_hooks::error::AiHookError;

/// Resolve the user's home directory, or surface a clean error.
///
/// Consults env vars (`HOME` on Unix, `USERPROFILE` then `HOME` on
/// Windows) before falling back to `dirs::home_dir()`. On Windows,
/// `dirs::home_dir()` calls `SHGetKnownFolderPath(FOLDERID_Profile)`
/// — a Win32 API that ignores env vars — so test sandboxes that
/// override `USERPROFILE` (e.g. `tests/ai_hooks_integration.rs`'s
/// `HomeGuard`) had no effect and the suite silently wrote agent
/// settings into the real user profile on every Windows tag-push CI
/// run since v0.2.0-rc.1. Preferring env vars makes the resolution
/// consistent with Unix behavior, respects user overrides, and
/// restores test-isolation correctness.
pub fn home_or_err() -> Result<PathBuf, AiHookError> {
    if let Some(home) = home_from_env() {
        return Ok(home);
    }
    dirs::home_dir().ok_or_else(|| {
        AiHookError::io(
            PathBuf::from("$HOME"),
            std::io::Error::other("cannot determine home directory"),
        )
    })
}

fn home_from_env() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        if let Some(p) = std::env::var_os("USERPROFILE").filter(|v| !v.is_empty()) {
            return Some(PathBuf::from(p));
        }
    }
    std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Read JSON from `path`. If the file is missing or empty, return an empty
/// object — the caller can treat it as "no prior config".
pub fn read_or_default_object(path: &Path) -> Result<Map<String, Value>, AiHookError> {
    if !path.exists() {
        return Ok(Map::new());
    }
    let bytes = fs::read(path).map_err(|e| AiHookError::io(path.to_path_buf(), e))?;
    // Early-exit on the first non-whitespace byte rather than scanning
    // the whole file. Settings files are tiny so this is just a cleaner
    // shape for the intent.
    if !bytes.iter().any(|b| !b.is_ascii_whitespace()) {
        return Ok(Map::new());
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|source| AiHookError::ParseExisting {
            path: path.to_path_buf(),
            source,
        })?;
    match value {
        Value::Object(map) => Ok(map),
        other => Err(AiHookError::InvalidEntry {
            name: path.display().to_string(),
            reason: format!(
                "expected top-level JSON object, found {}",
                json_kind(&other)
            ),
        }),
    }
}

/// Atomic write: serialize to a sibling tempfile (PID + nanos suffix so
/// concurrent runs don't clash), fsync, then rename. Compact JSON; agent
/// settings are consumed by daemons, not humans — pretty-printing wastes
/// ~2× the bytes for zero benefit.
pub fn write_json(path: &Path, value: &Value) -> Result<(), AiHookError> {
    refuse_if_symlink(path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| AiHookError::io(parent.to_path_buf(), e))?;
    }
    let bytes = serde_json::to_vec(value)?;
    let tmp_path = tempfile_path(path);
    write_tempfile_then_rename(&tmp_path, path, &bytes, 0o644)
}

/// Write an executable text file (Cline fragments + dispatcher). Atomic,
/// 0o755 on Unix.
pub fn write_executable(path: &Path, body: &str) -> Result<(), AiHookError> {
    refuse_if_symlink(path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| AiHookError::io(parent.to_path_buf(), e))?;
    }
    let tmp_path = tempfile_path(path);
    write_tempfile_then_rename(&tmp_path, path, body.as_bytes(), 0o755)
}

/// Atomic text write. Used by Continue's YAML provisioner so it gets the
/// same crash-safety as the JSON-shaped agents.
pub fn write_text_atomic(path: &Path, body: &str) -> Result<(), AiHookError> {
    refuse_if_symlink(path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| AiHookError::io(parent.to_path_buf(), e))?;
    }
    let tmp_path = tempfile_path(path);
    write_tempfile_then_rename(&tmp_path, path, body.as_bytes(), 0o644)
}

fn refuse_if_symlink(path: &Path) -> Result<(), AiHookError> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(AiHookError::SettingsPathIsSymlink {
            path: path.to_path_buf(),
        }),
        _ => Ok(()),
    }
}

fn tempfile_path(path: &Path) -> PathBuf {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    path.with_extension(format!("jarvy.tmp.{pid}.{nanos}"))
}

fn write_tempfile_then_rename(
    tmp_path: &Path,
    final_path: &Path,
    bytes: &[u8],
    #[cfg_attr(not(unix), allow(unused_variables))] mode: u32,
) -> Result<(), AiHookError> {
    {
        let mut opts = fs::OpenOptions::new();
        opts.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(mode);
        }
        let mut tmp = opts
            .open(tmp_path)
            .map_err(|e| AiHookError::io(tmp_path.to_path_buf(), e))?;
        tmp.write_all(bytes)
            .map_err(|e| AiHookError::io(tmp_path.to_path_buf(), e))?;
        tmp.sync_all()
            .map_err(|e| AiHookError::io(tmp_path.to_path_buf(), e))?;
    }
    fs::rename(tmp_path, final_path).map_err(|e| AiHookError::io(final_path.to_path_buf(), e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(final_path, fs::Permissions::from_mode(mode));
    }
    Ok(())
}

fn json_kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn read_missing_returns_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let map = read_or_default_object(&path).unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn read_whitespace_only_returns_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, b"   \n\t  \n").unwrap();
        let map = read_or_default_object(&path).unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn write_then_read_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");
        let value = json!({ "hooks": { "PreToolUse": [] } });
        write_json(&path, &value).unwrap();
        let read = read_or_default_object(&path).unwrap();
        assert!(read.contains_key("hooks"));
    }

    #[test]
    fn read_rejects_non_object_root() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, b"[1,2,3]").unwrap();
        let err = read_or_default_object(&path).unwrap_err();
        assert!(matches!(err, AiHookError::InvalidEntry { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn write_executable_sets_mode_0755() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let path = dir.path().join("hook");
        write_executable(&path, "#!/bin/sh\nexit 0\n").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755);
    }

    #[cfg(unix)]
    #[test]
    fn refuses_to_write_through_symlink() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("real-file");
        fs::write(&target, b"original").unwrap();
        let link = dir.path().join("settings.json");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let err = write_json(&link, &json!({"hooks": {}})).unwrap_err();
        assert!(matches!(err, AiHookError::SettingsPathIsSymlink { .. }));
        // Target unchanged.
        assert_eq!(fs::read(&target).unwrap(), b"original");
    }

    #[test]
    fn tempfile_path_includes_pid_and_nanos() {
        let p = std::path::Path::new("/tmp/foo.json");
        let t = tempfile_path(p);
        let name = t.file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with("foo.jarvy.tmp."));
        // Two characters minimum after the "tmp." prefix
        assert!(name.len() > "foo.jarvy.tmp.".len() + 2);
    }
}
