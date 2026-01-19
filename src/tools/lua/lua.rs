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
    fn ensure_lua_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
