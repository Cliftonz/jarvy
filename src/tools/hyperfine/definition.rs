//! hyperfine - command-line benchmarking tool
//!
//! hyperfine is a command-line benchmarking tool inspired by bench.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(HYPERFINE, {
    command: "hyperfine",
    repo: "sharkdp/hyperfine",
    macos: { brew: "hyperfine" },
    linux: { apt: "hyperfine", dnf: "hyperfine", pacman: "hyperfine", apk: "hyperfine" },
    windows: { winget: "sharkdp.hyperfine", choco: "hyperfine" },
    bsd: { pkg: "hyperfine" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hyperfine_registration_shape() {
        assert_eq!(HYPERFINE.command, "hyperfine");
        let mac = HYPERFINE.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("hyperfine"));
        let win = HYPERFINE.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("sharkdp.hyperfine"));
    }
}
