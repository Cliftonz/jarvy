//! zig - a systems programming language
//!
//! Zig is a general-purpose programming language and build system.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ZIG, {
    command: "zig",
    macos: { brew: "zig" },
    linux: { apt: "zig", dnf: "zig", pacman: "zig", apk: "zig" },
    windows: { winget: "zig.zig", choco: "zig" },
    bsd: { pkg: "zig" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_zig_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
