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
    fn nim_registration_shape() {
        assert_eq!(NIM.command, "nim");
        let mac = NIM.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("nim"));
        let win = NIM.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Nim.Nim"));
    }
}
