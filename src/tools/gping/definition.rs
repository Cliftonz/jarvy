//! gping - ping with a graph
//!
//! Gping is a ping utility with a real-time graph. It supports pinging
//! multiple hosts simultaneously and displays a visual representation
//! of latency over time.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GPING, {
    command: "gping",
    repo: "orf/gping",
    macos: { brew: "gping" },
    linux: { uniform: "gping" },
    windows: { winget: "orf.gping" },
    bsd: { pkg: "gping" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gping_registration_shape() {
        assert_eq!(GPING.command, "gping");
        let mac = GPING.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("gping"));
        let win = GPING.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("orf.gping"));
    }
}
