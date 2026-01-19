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
    fn ensure_julia_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
