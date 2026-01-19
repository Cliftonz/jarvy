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
    fn ensure_luarocks_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
