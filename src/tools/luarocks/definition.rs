//! luarocks - package manager for Lua modules
//!
//! LuaRocks is the package manager for Lua modules. It allows you to
//! create and install Lua modules as self-contained packages called rocks.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(LUAROCKS, {
    command: "luarocks",
    macos: { brew: "luarocks" },
    linux: { apt: "luarocks", dnf: "luarocks", pacman: "luarocks", apk: "luarocks" },
    windows: { winget: "LuaRocks.LuaRocks" },
    bsd: { pkg: "luarocks" },
    depends_on: &["lua"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luarocks_registration_shape() {
        assert_eq!(LUAROCKS.command, "luarocks");
        let mac = LUAROCKS.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("luarocks"));
        let win = LUAROCKS.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("LuaRocks.LuaRocks"));
    }
}
