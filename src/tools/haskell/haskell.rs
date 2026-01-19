//! haskell - purely functional programming language (via GHCup)
//!
//! Haskell is an advanced, purely functional programming language.
//! This installs GHCup, the recommended Haskell installer.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(HASKELL, {
    command: "ghc",
    macos: { brew: "ghc" },
    linux: { apt: "ghc", dnf: "ghc", pacman: "ghc", apk: "ghc" },
    windows: { winget: "Haskell.GHCup" },
    bsd: { pkg: "ghc" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_haskell_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
