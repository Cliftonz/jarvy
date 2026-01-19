//! k6 - load testing tool by Grafana
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(K6, {
    command: "k6",
    macos: { brew: "k6" },
    linux: { uniform: "k6" },
    windows: { winget: "Grafana.k6" },
    bsd: { pkg: "k6" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_k6_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
