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
    fn haskell_registration_shape() {
        assert_eq!(HASKELL.command, "ghc");
        let mac = HASKELL.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("ghc"));
        let win = HASKELL.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Haskell.GHCup"));
    }
}
