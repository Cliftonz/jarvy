//! julia - high-level dynamic programming language
//!
//! Julia is a high-level, high-performance dynamic language for technical
//! computing with syntax familiar to users of other technical computing
//! environments (MATLAB, Python, R).
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(JULIA, {
    command: "julia",
    macos: { cask: "julia" },
    linux: { apt: "julia", dnf: "julia", pacman: "julia", apk: "julia" },
    windows: { winget: "Julialang.Julia" },
    bsd: { pkg: "julia" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn julia_registration_shape() {
        assert_eq!(JULIA.command, "julia");
        let mac = JULIA.macos.expect("must support macOS");
        assert_eq!(mac.cask, Some("julia"));
        let win = JULIA.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Julialang.Julia"));
    }
}
