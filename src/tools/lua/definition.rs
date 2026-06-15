//! lua - lightweight scripting language
//!
//! Lua is a powerful, efficient, lightweight, embeddable scripting language.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(LUA, {
    command: "lua",
    macos: { brew: "lua" },
    linux: { apt: "lua5.4", dnf: "lua", pacman: "lua", apk: "lua" },
    windows: { winget: "DEVCOM.Lua", choco: "lua" },
    bsd: { pkg: "lua54" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lua_registration_shape() {
        assert_eq!(LUA.command, "lua");
        let mac = LUA.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("lua"));
        let win = LUA.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("DEVCOM.Lua"));
    }
}
