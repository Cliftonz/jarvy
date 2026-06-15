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
    fn zig_registration_shape() {
        assert_eq!(ZIG.command, "zig");
        let mac = ZIG.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("zig"));
        let win = ZIG.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("zig.zig"));
    }
}
