//! mimirtool - Grafana Mimir admin CLI
//!
//! mimirtool interacts with Grafana Mimir (and Cortex) clusters:
//! rules, alertmanager configs, tenant analysis, backfill helpers.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(MIMIRTOOL, {
    command: "mimirtool",
    // The `grafana/grafana` tap is auto-tapped by jarvy on install
    // (two-slash brew form triggers `brew tap grafana/grafana` first).
    macos: { brew: "grafana/grafana/mimirtool" },
    // No first-party apt/dnf or winget package as of 2026-08.
    // Linux + Windows fall through to the go fallback.
    fallback: { go: "github.com/grafana/mimir/cmd/mimirtool" },
    category: "observability",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mimirtool_registration_shape() {
        assert_eq!(MIMIRTOOL.command, "mimirtool");
        assert_eq!(MIMIRTOOL.category, Some("observability"));
        let mac = MIMIRTOOL.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("grafana/grafana/mimirtool"));
        assert!(MIMIRTOOL.linux.is_none());
        assert!(MIMIRTOOL.windows.is_none());
        assert_eq!(MIMIRTOOL.fallback.len(), 1);
        assert_eq!(
            MIMIRTOOL.fallback[0].eco,
            crate::tools::spec::FallbackEco::Go
        );
        assert_eq!(
            MIMIRTOOL.fallback[0].package,
            "github.com/grafana/mimir/cmd/mimirtool"
        );
    }
}
