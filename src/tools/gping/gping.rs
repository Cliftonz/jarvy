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
    macos: { brew: "gping" },
    linux: { uniform: "gping" },
    windows: { winget: "orf.gping" },
    bsd: { pkg: "gping" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_gping_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
