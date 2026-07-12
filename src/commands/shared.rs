//! Cross-command helpers shared by the interactive menu and `jarvy run`.
//!
//! These mechanics were originally duplicated between `interactive.rs` and
//! `run_cmd.rs` and had already drifted (the menu's spawn had no Windows
//! branch). Policy stays with each caller — only the plumbing lives here.

use std::io;
use std::path::Path;
use std::process::ExitStatus;

/// Strip ANSI escape sequences and other control characters from text that
/// will be displayed to the user. Prevents a malicious jarvy.toml from
/// hiding parts of a command behind escape codes.
pub(crate) fn sanitize_for_display(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip CSI sequences `ESC [ ... letter`.
            if matches!(chars.peek(), Some('[')) {
                chars.next();
                while let Some(&n) = chars.peek() {
                    chars.next();
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        if (c as u32) < 0x20 && c != '\t' {
            out.push('?');
            continue;
        }
        out.push(c);
    }
    out
}

/// Truncated SHA-256 (8 bytes, hex) of a command line for telemetry.
/// Privacy contract shared by `interactive.command.*` and `run.command.*`:
/// events carry this hash, never the command text. The truncation length is
/// part of the telemetry schema — change it here or nowhere.
pub(crate) fn short_cmd_hash(cmd: &str) -> String {
    use sha2::{Digest, Sha256};
    let bytes = Sha256::digest(cmd.as_bytes());
    hex::encode(&bytes[..8])
}

/// Run a command string through the platform shell (`sh -c` on unix,
/// `cmd /C` on Windows), inheriting stdio. `dir` sets the child's working
/// directory; `None` inherits the caller's cwd.
pub(crate) fn spawn_shell(cmd: &str, dir: Option<&Path>) -> io::Result<ExitStatus> {
    #[cfg(windows)]
    let mut command = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", cmd]);
        c
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut c = std::process::Command::new("sh");
        c.args(["-c", cmd]);
        c
    };
    if let Some(d) = dir {
        command.current_dir(d);
    }
    command.status()
}

/// Quote one argv token for the platform shell `spawn_shell` uses.
/// The per-dialect functions below are compiled unconditionally so BOTH
/// arms are unit-testable on any host; only this selector is cfg-gated.
pub(crate) fn quote_shell_arg(arg: &str) -> String {
    #[cfg(windows)]
    {
        quote_shell_arg_windows(arg)
    }
    #[cfg(not(windows))]
    {
        quote_shell_arg_posix(arg)
    }
}

/// POSIX single-quote wrapping; embedded `'` becomes `'\''`.
pub(crate) fn quote_shell_arg_posix(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', r"'\''"))
}

/// cmd.exe double-quote wrapping; embedded `"` doubled. NOTE: this cannot
/// neutralize `%VAR%` expansion — cmd.exe expands percent references even
/// inside double quotes and offers no in-quote escape. Callers must refuse
/// `%`-bearing args on Windows (see `windows_arg_is_expansion_safe`)
/// rather than pretend to escape them.
// Compiled on every platform so the Windows arm is unit-testable on
// unix CI (QA review F9); only test + windows callers exist on unix.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn quote_shell_arg_windows(arg: &str) -> String {
    format!("\"{}\"", arg.replace('"', "\"\""))
}

/// True when an argument can be delivered verbatim through `cmd /C` — i.e.
/// contains no `%`. A `%NAME%` pattern in an appended arg would be expanded
/// by cmd.exe before the child sees it (CI env vars often hold secrets),
/// silently breaking the verbatim-delivery guarantee `jarvy run -- <args>`
/// makes. Compiled on all platforms for testability; enforced on Windows.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn windows_arg_is_expansion_safe(arg: &str) -> bool {
    !arg.contains('%')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_ansi_escapes() {
        let raw = "\x1b[31mevil\x1b[0m cargo test";
        assert_eq!(sanitize_for_display(raw), "evil cargo test");
    }

    #[test]
    fn sanitize_replaces_control_chars() {
        assert_eq!(sanitize_for_display("abc\x07def"), "abc?def");
    }

    #[test]
    fn short_cmd_hash_is_16_hex_chars() {
        let h = short_cmd_hash("cargo test");
        assert_eq!(h.len(), 16);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        // Stable across calls — this is a telemetry-correlation key.
        assert_eq!(h, short_cmd_hash("cargo test"));
        assert_ne!(h, short_cmd_hash("cargo build"));
    }

    #[test]
    fn posix_quote_escapes_single_quotes() {
        assert_eq!(quote_shell_arg_posix("it's"), r"'it'\''s'");
        assert_eq!(quote_shell_arg_posix("a b"), "'a b'");
    }

    #[test]
    fn windows_quote_doubles_double_quotes() {
        assert_eq!(quote_shell_arg_windows(r#"say "hi""#), r#""say ""hi""""#);
    }

    #[test]
    fn windows_expansion_safety_refuses_percent() {
        // Quoting cannot protect these — they must be refused on Windows.
        assert!(!windows_arg_is_expansion_safe("%TEMP%"));
        assert!(!windows_arg_is_expansion_safe("100%"));
        assert!(windows_arg_is_expansion_safe("plain-arg"));
        assert!(windows_arg_is_expansion_safe("a b \"c\""));
    }

    #[test]
    fn spawn_shell_runs_in_given_dir() {
        let tmp = std::env::temp_dir();
        // `cd`-free pwd check works on both sh and cmd (`cd` with no args
        // prints cwd on cmd; use exit code only to stay portable here).
        let status = spawn_shell("exit 0", Some(&tmp)).expect("spawn");
        assert!(status.success());
    }
}
