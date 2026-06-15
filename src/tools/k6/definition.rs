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
    fn k6_registration_shape() {
        assert_eq!(K6.command, "k6");
        let mac = K6.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("k6"));
        let win = K6.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Grafana.k6"));
    }
}
