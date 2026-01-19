//! nim - statically typed compiled systems language
//!
//! Nim is a statically typed compiled systems programming language that
//! combines successful concepts from mature languages like Python, Ada
//! and Modula.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(NIM, {
    command: "nim",
    macos: { brew: "nim" },
    linux: { apt: "nim", dnf: "nim", pacman: "nim", apk: "nim" },
    windows: { winget: "Nim.Nim" },
    bsd: { pkg: "nim" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_nim_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
