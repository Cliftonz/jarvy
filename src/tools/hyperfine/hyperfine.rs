//! hyperfine - command-line benchmarking tool
//!
//! hyperfine is a command-line benchmarking tool inspired by bench.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(HYPERFINE, {
    command: "hyperfine",
    macos: { brew: "hyperfine" },
    linux: { apt: "hyperfine", dnf: "hyperfine", pacman: "hyperfine", apk: "hyperfine" },
    windows: { winget: "sharkdp.hyperfine", choco: "hyperfine" },
    bsd: { pkg: "hyperfine" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_hyperfine_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
