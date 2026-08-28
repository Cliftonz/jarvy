//! PATHEXT edit — add `.SH` to the user's executable-extensions list.
//!
//! Two halves. `compute_new_pathext` is pure and cross-platform so the
//! semantics (case-insensitive dedup, `;`-separator preservation,
//! stable ordering) are unit-testable off Windows. The executor
//! `apply_on_windows` is `cfg(target_os = "windows")` and shells out
//! to `reg add HKCU\Environment` with the `REG_EXPAND_SZ` type so the
//! new PATHEXT keeps its `%FOO%` expansions.

// The pure planner + constants below are consumed by the Windows
// `exec` submodule AND by unit tests. On a non-Windows release build
// dead-code analysis sees zero readers because `exec` is cfg-gated
// out. The allow silences that specific class of false positive.
#![allow(dead_code)]

/// Compute the new PATHEXT string. Returns `None` when the extension
/// is already present in any casing (idempotent — reruns of `jarvy
/// setup` do not re-emit the change event).
///
/// `.SH` is appended in upper case to match the Windows convention
/// established by the default PATHEXT (`.COM;.EXE;.BAT;.CMD;.VBS;
/// .VBE;.JS;.JSE;.WSF;.WSH;.MSC`). Semicolon separator, no leading /
/// trailing separator on the returned value.
pub fn compute_new_pathext(current: &str, ext_to_add: &str) -> Option<String> {
    // Normalize the addition to `.EXT` upper-case with a leading dot.
    let target = normalize_ext(ext_to_add);
    let existing: Vec<&str> = current
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let already_present = existing.iter().any(|e| e.eq_ignore_ascii_case(&target));
    if already_present {
        return None;
    }
    let mut updated: Vec<String> = existing.iter().map(|s| s.to_string()).collect();
    updated.push(target);
    Some(updated.join(";"))
}

/// Normalize `sh` / `.sh` / `.SH` → `.SH`.
fn normalize_ext(ext: &str) -> String {
    let trimmed = ext.trim().trim_start_matches('.');
    format!(".{}", trimmed.to_ascii_uppercase())
}

/// Default PATHEXT Windows ships with, used as the seed when the
/// user's HKCU Environment has no PATHEXT set. Values from a fresh
/// Windows 11 install — matches `cmd /c echo %PATHEXT%` before any
/// user override.
pub const WINDOWS_DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC";

#[cfg(target_os = "windows")]
mod exec {
    use std::process::Command;

    /// Outcome of the PATHEXT-apply pass.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PathExtOutcome {
        /// `.SH` was appended and HKCU\Environment\PATHEXT written.
        Applied,
        /// `.SH` was already present in the effective PATHEXT.
        Unchanged,
    }

    /// Read the user's PATHEXT via `reg query`. Returns `None` if the
    /// value is absent (fresh user profile) so the caller can seed
    /// from [`super::WINDOWS_DEFAULT_PATHEXT`] rather than starting
    /// from an empty string (which would REPLACE the system PATHEXT
    /// for that user — a genuinely dangerous outcome).
    pub fn read_hkcu_pathext() -> std::io::Result<Option<String>> {
        let output = Command::new("reg")
            .args(["query", "HKCU\\Environment", "/v", "PATHEXT"])
            .output()?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(parse_reg_query_value(&String::from_utf8_lossy(
            &output.stdout,
        )))
    }

    /// Parse `PATHEXT    REG_EXPAND_SZ    .COM;.EXE;...` out of
    /// `reg query` stdout. Robust to leading whitespace and the two
    /// type-name variants the tool emits.
    pub fn parse_reg_query_value(stdout: &str) -> Option<String> {
        for line in stdout.lines() {
            let line = line.trim_start();
            if !line.starts_with("PATHEXT") {
                continue;
            }
            for token in ["REG_EXPAND_SZ", "REG_SZ"] {
                if let Some(idx) = line.find(token) {
                    let value = line[idx + token.len()..].trim();
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
            }
        }
        None
    }

    /// Apply the PATHEXT edit. Reads HKCU, seeds from the system
    /// default if unset, dedups case-insensitively, writes back
    /// via `reg add /t REG_EXPAND_SZ` so `%VAR%` expansions inside
    /// the value keep working.
    pub fn apply(ext_to_add: &str) -> std::io::Result<PathExtOutcome> {
        let current =
            read_hkcu_pathext()?.unwrap_or_else(|| super::WINDOWS_DEFAULT_PATHEXT.to_string());
        let Some(new_value) = super::compute_new_pathext(&current, ext_to_add) else {
            return Ok(PathExtOutcome::Unchanged);
        };
        let status = Command::new("reg")
            .args([
                "add",
                "HKCU\\Environment",
                "/v",
                "PATHEXT",
                "/t",
                "REG_EXPAND_SZ",
                "/d",
                &new_value,
                "/f",
            ])
            .status()?;
        if !status.success() {
            return Err(std::io::Error::other(format!(
                "reg add HKCU\\Environment\\PATHEXT exited with {}",
                status.code().unwrap_or(-1)
            )));
        }
        Ok(PathExtOutcome::Applied)
    }
}

#[cfg(target_os = "windows")]
#[allow(unused_imports)]
// Consumed by cross-platform integration tests + Windows exec.
pub use exec::{PathExtOutcome, apply, parse_reg_query_value};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_accepts_bare_extension() {
        assert_eq!(normalize_ext("sh"), ".SH");
    }

    #[test]
    fn normalize_accepts_dotted_extension() {
        assert_eq!(normalize_ext(".sh"), ".SH");
    }

    #[test]
    fn normalize_uppercases() {
        assert_eq!(normalize_ext(".sh"), ".SH");
        assert_eq!(normalize_ext(".Sh"), ".SH");
        assert_eq!(normalize_ext(".SH"), ".SH");
    }

    #[test]
    fn compute_appends_when_absent() {
        let updated = compute_new_pathext(".COM;.EXE;.BAT", ".sh").unwrap();
        assert_eq!(updated, ".COM;.EXE;.BAT;.SH");
    }

    #[test]
    fn compute_returns_none_when_already_present_uppercase() {
        assert!(compute_new_pathext(".COM;.EXE;.SH", ".sh").is_none());
    }

    #[test]
    fn compute_returns_none_when_already_present_lowercase() {
        // A user who added `.sh` by hand shouldn't get a duplicated
        // `.SH` entry on next setup — dedup is case-insensitive.
        assert!(compute_new_pathext(".COM;.exe;.sh", ".SH").is_none());
    }

    #[test]
    fn compute_preserves_existing_order() {
        let updated = compute_new_pathext(".WSH;.COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS", ".SH").unwrap();
        assert_eq!(updated, ".WSH;.COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.SH");
    }

    #[test]
    fn compute_handles_empty_input() {
        // An empty PATHEXT would be replaced entirely — the executor
        // guards against this by seeding from WINDOWS_DEFAULT_PATHEXT
        // when HKCU has no value, so the planner never sees "".
        // Test the pure function's behavior for completeness.
        let updated = compute_new_pathext("", ".sh").unwrap();
        assert_eq!(updated, ".SH");
    }

    #[test]
    fn compute_ignores_stray_semicolons() {
        // `.COM;;.EXE` shouldn't produce a phantom empty segment.
        let updated = compute_new_pathext(".COM;;.EXE", ".sh").unwrap();
        assert_eq!(updated, ".COM;.EXE;.SH");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn parses_reg_query_pathext_line() {
        let sample = "\r\nHKEY_CURRENT_USER\\Environment\r\n    PATHEXT    REG_EXPAND_SZ    .COM;.EXE;.BAT;.CMD\r\n";
        assert_eq!(
            parse_reg_query_value(sample).as_deref(),
            Some(".COM;.EXE;.BAT;.CMD")
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn parses_reg_query_reg_sz_variant() {
        let sample = "    PATHEXT    REG_SZ    .COM;.EXE";
        assert_eq!(parse_reg_query_value(sample).as_deref(), Some(".COM;.EXE"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn parses_reg_query_returns_none_on_missing() {
        assert!(parse_reg_query_value("ERROR: The system was unable to find...").is_none());
    }
}
