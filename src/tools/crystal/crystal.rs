//! crystal - Ruby-like language with static typing
//!
//! Crystal is a programming language with the following goals:
//! - Have syntax similar to Ruby
//! - Statically type-checked, but without having to specify types
//! - Compile to efficient native code
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(CRYSTAL, {
    command: "crystal",
    macos: { brew: "crystal" },
    linux: { apt: "crystal", dnf: "crystal", pacman: "crystal", apk: "crystal" },
    bsd: { pkg: "crystal" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_crystal_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
