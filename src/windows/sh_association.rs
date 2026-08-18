//! `.sh` file association — route Explorer double-click / `start` to
//! `bash.exe`. All writes are per-user (HKCU\Software\Classes\...),
//! no admin needed and no impact on other Windows accounts.
//!
//! Only the `"open"` verb is routed. The `"edit"` verb is left alone so
//! Shift+right-click → Edit still opens `.sh` in the user's chosen
//! editor (VS Code, Notepad++, whatever the shell had before).

// Planner + registry-shape items are consumed by the Windows exec
// submodule AND by tests; dead-code analysis on non-Windows non-test
// builds sees no readers. Silence that specific false positive here
// rather than sprinkling allows on every item.
#![allow(dead_code)]

use super::config::ShAssociationMode;

/// Standard Git for Windows install paths, checked in order. First one
/// that exists on disk wins. WOW64 mirror covered so a 32-bit Git for
/// Windows install on a 64-bit OS is also found.
///
/// Order matters — put the modern path first so a machine with both
/// installed prefers the current release.
const DEFAULT_BASH_PATHS: &[&str] = &[
    "C:\\Program Files\\Git\\bin\\bash.exe",
    "C:\\Program Files (x86)\\Git\\bin\\bash.exe",
];

/// Resolved bash launcher command. `program` goes to
/// `reg add ... /d "\"program\" \"%1\" %*"` — the space between `"%1"`
/// and `%*` mirrors what `ftype cmd` writes for the built-in
/// associations and is what Explorer expects on double-click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BashLauncher {
    pub program: String,
}

impl BashLauncher {
    /// Command string handed to the registry — the shape Windows uses
    /// as `HKCR\<ProgId>\shell\open\command\(Default)`.
    pub fn command_line(&self) -> String {
        format!("\"{}\" \"%1\" %*", self.program)
    }
}

/// Plan the association write. Returns `None` when the mode is `Off`
/// so the caller can no-op cleanly.
///
/// `bash_path_override` (from the user's `[windows]` block) wins over
/// [`DEFAULT_BASH_PATHS`]; a probe fn is injected so tests don't need
/// to touch the filesystem.
pub fn plan_association(
    mode: ShAssociationMode,
    bash_path_override: Option<&str>,
    exists: &dyn Fn(&str) -> bool,
) -> Result<Option<BashLauncher>, AssociationError> {
    if matches!(mode, ShAssociationMode::Off) {
        return Ok(None);
    }
    if let Some(override_path) = bash_path_override {
        // Path safety was validated at config-load
        // (`config::validate_bash_path`) — this is defense in depth
        // for the executor path.
        if override_path.contains('"')
            || override_path.contains('\n')
            || override_path.contains('\0')
        {
            return Err(AssociationError::UnsafeBashPath);
        }
        if !exists(override_path) {
            return Err(AssociationError::OverrideNotFound(
                override_path.to_string(),
            ));
        }
        return Ok(Some(BashLauncher {
            program: override_path.to_string(),
        }));
    }
    for candidate in DEFAULT_BASH_PATHS {
        if exists(candidate) {
            return Ok(Some(BashLauncher {
                program: (*candidate).to_string(),
            }));
        }
    }
    Err(AssociationError::BashNotFound)
}

/// Errors surfaced by [`plan_association`] and the Windows executor.
#[derive(Debug, thiserror::Error)]
pub enum AssociationError {
    #[error(
        "bash.exe not found in any standard Git for Windows path; \
         install Git for Windows or set `[windows] bash_path_override`"
    )]
    BashNotFound,

    #[error("bash_path_override `{0}` does not exist on disk")]
    OverrideNotFound(String),

    #[error("bash_path_override contains characters unsafe for registry write")]
    UnsafeBashPath,

    #[error("reg add exited with status {0}")]
    RegAddFailed(i32),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// The registry-key layout jarvy writes for `.sh` → bash. Split out as
/// a struct so tests can assert the shape without executing anything.
///
/// - `\Software\Classes\.sh\(Default)` → `Jarvy.BashScript`
///   (the extension's ProgId — points at the class below)
/// - `\Software\Classes\Jarvy.BashScript\shell\open\command\(Default)`
///   → `"C:\Program Files\Git\bin\bash.exe" "%1" %*`
///
/// Both writes are HKCU-scoped. `Jarvy.BashScript` is a distinct
/// ProgId so a future `remove` command can idempotently clean up
/// jarvy's writes without touching a user's pre-existing custom class.
pub struct RegistryPlan<'a> {
    pub extension_key: &'a str,
    pub extension_value: &'a str,
    pub prog_id_key: String,
    pub prog_id_command: String,
}

impl<'a> RegistryPlan<'a> {
    pub const PROG_ID: &'static str = "Jarvy.BashScript";
    pub const EXTENSION_KEY: &'static str = "HKCU\\Software\\Classes\\.sh";
    pub const PROG_ID_KEY_PREFIX: &'static str = "HKCU\\Software\\Classes\\";

    pub fn for_launcher(launcher: &BashLauncher) -> Self {
        Self {
            extension_key: Self::EXTENSION_KEY,
            extension_value: Self::PROG_ID,
            prog_id_key: format!(
                "{}{}\\shell\\open\\command",
                Self::PROG_ID_KEY_PREFIX,
                Self::PROG_ID
            ),
            prog_id_command: launcher.command_line(),
        }
    }
}

#[cfg(target_os = "windows")]
mod exec {
    use super::{AssociationError, BashLauncher, RegistryPlan};
    use std::process::Command;

    /// Outcome of the association-apply pass.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AssociationOutcome {
        /// Registry keys were written.
        Applied,
        /// Both keys already had the target values — no writes needed.
        Unchanged,
    }

    /// Read the current `(Default)` value at a registry key, `None`
    /// when the key is absent or the value is empty.
    fn read_default(key: &str) -> std::io::Result<Option<String>> {
        let output = Command::new("reg").args(["query", key, "/ve"]).output()?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(parse_default_value(&String::from_utf8_lossy(
            &output.stdout,
        )))
    }

    pub fn parse_default_value(stdout: &str) -> Option<String> {
        for line in stdout.lines() {
            let line = line.trim_start();
            if !line.starts_with("(Default)") {
                continue;
            }
            for token in ["REG_SZ", "REG_EXPAND_SZ"] {
                if let Some(idx) = line.find(token) {
                    let value = line[idx + token.len()..].trim();
                    if !value.is_empty() && value != "(value not set)" {
                        return Some(value.to_string());
                    }
                }
            }
        }
        None
    }

    /// Write a `(Default)` value at `key` via `reg add`. `/f` skips
    /// the interactive overwrite prompt; type is `REG_SZ` — the class
    /// command line doesn't need env-var expansion since we resolved
    /// bash.exe at plan time.
    fn write_default(key: &str, value: &str) -> Result<(), AssociationError> {
        let status = Command::new("reg")
            .args(["add", key, "/ve", "/t", "REG_SZ", "/d", value, "/f"])
            .status()?;
        if !status.success() {
            return Err(AssociationError::RegAddFailed(status.code().unwrap_or(-1)));
        }
        Ok(())
    }

    /// Apply the association write per the plan. Idempotent: skips
    /// each key whose current value already matches the target.
    pub fn apply(launcher: &BashLauncher) -> Result<AssociationOutcome, AssociationError> {
        let plan = RegistryPlan::for_launcher(launcher);

        let existing_ext = read_default(plan.extension_key)?;
        let existing_cmd = read_default(&plan.prog_id_key)?;
        let ext_matches = existing_ext.as_deref() == Some(plan.extension_value);
        let cmd_matches = existing_cmd.as_deref() == Some(plan.prog_id_command.as_str());

        if ext_matches && cmd_matches {
            return Ok(AssociationOutcome::Unchanged);
        }

        if !ext_matches {
            write_default(plan.extension_key, plan.extension_value)?;
        }
        if !cmd_matches {
            write_default(&plan.prog_id_key, &plan.prog_id_command)?;
        }
        Ok(AssociationOutcome::Applied)
    }
}

#[cfg(target_os = "windows")]
pub use exec::{AssociationOutcome, apply, parse_default_value};

#[cfg(test)]
mod tests {
    use super::*;

    fn no_files_exist(_: &str) -> bool {
        false
    }

    fn only_default_bash_exists(path: &str) -> bool {
        path == "C:\\Program Files\\Git\\bin\\bash.exe"
    }

    #[test]
    fn plan_returns_none_when_mode_is_off() {
        let plan = plan_association(ShAssociationMode::Off, None, &no_files_exist).unwrap();
        assert!(plan.is_none());
    }

    #[test]
    fn plan_finds_default_git_for_windows_install() {
        let launcher = plan_association(ShAssociationMode::Open, None, &only_default_bash_exists)
            .unwrap()
            .expect("launcher resolved");
        assert_eq!(launcher.program, "C:\\Program Files\\Git\\bin\\bash.exe");
    }

    #[test]
    fn plan_prefers_override_when_provided() {
        let launcher = plan_association(
            ShAssociationMode::Open,
            Some("D:\\custom\\bash.exe"),
            &|p| p == "D:\\custom\\bash.exe",
        )
        .unwrap()
        .expect("launcher resolved");
        assert_eq!(launcher.program, "D:\\custom\\bash.exe");
    }

    #[test]
    fn plan_errors_when_override_missing() {
        let err = plan_association(
            ShAssociationMode::Open,
            Some("D:\\nope\\bash.exe"),
            &no_files_exist,
        )
        .unwrap_err();
        matches!(err, AssociationError::OverrideNotFound(_));
    }

    #[test]
    fn plan_errors_when_no_bash_found() {
        let err = plan_association(ShAssociationMode::Open, None, &no_files_exist).unwrap_err();
        matches!(err, AssociationError::BashNotFound);
    }

    #[test]
    fn plan_refuses_override_with_unsafe_chars() {
        let err = plan_association(ShAssociationMode::Open, Some("C:\\bash\".exe"), &|_| true)
            .unwrap_err();
        matches!(err, AssociationError::UnsafeBashPath);
    }

    #[test]
    fn command_line_quotes_program_and_percent1() {
        let launcher = BashLauncher {
            program: "C:\\Program Files\\Git\\bin\\bash.exe".to_string(),
        };
        assert_eq!(
            launcher.command_line(),
            "\"C:\\Program Files\\Git\\bin\\bash.exe\" \"%1\" %*"
        );
    }

    #[test]
    fn registry_plan_uses_stable_prog_id() {
        let launcher = BashLauncher {
            program: "C:\\Program Files\\Git\\bin\\bash.exe".to_string(),
        };
        let plan = RegistryPlan::for_launcher(&launcher);
        assert_eq!(plan.extension_key, "HKCU\\Software\\Classes\\.sh");
        assert_eq!(plan.extension_value, "Jarvy.BashScript");
        assert_eq!(
            plan.prog_id_key,
            "HKCU\\Software\\Classes\\Jarvy.BashScript\\shell\\open\\command"
        );
        assert_eq!(
            plan.prog_id_command,
            "\"C:\\Program Files\\Git\\bin\\bash.exe\" \"%1\" %*"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn parses_default_value_from_reg_query_output() {
        let sample = "\r\nHKEY_CURRENT_USER\\Software\\Classes\\.sh\r\n    (Default)    REG_SZ    Jarvy.BashScript\r\n";
        assert_eq!(
            parse_default_value(sample).as_deref(),
            Some("Jarvy.BashScript")
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn parses_default_returns_none_when_unset() {
        let sample = "    (Default)    REG_SZ    (value not set)";
        assert!(parse_default_value(sample).is_none());
    }
}
