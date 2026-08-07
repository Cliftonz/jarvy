//! promtail - Grafana Loki log shipper
//!
//! promtail tails files, discovers targets, and pushes log lines to a
//! Loki endpoint. Upstream now recommends Grafana Alloy over promtail
//! for new deployments; both remain supported.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(PROMTAIL, {
    command: "promtail",
    // No first-party brew, apt/dnf, or winget package as of 2026-08.
    // The go fallback covers every platform.
    fallback: { go: "github.com/grafana/loki/v3/clients/cmd/promtail" },
    category: "observability",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promtail_registration_shape() {
        assert_eq!(PROMTAIL.command, "promtail");
        assert_eq!(PROMTAIL.category, Some("observability"));
        assert!(PROMTAIL.macos.is_none());
        assert!(PROMTAIL.linux.is_none());
        assert!(PROMTAIL.windows.is_none());
        assert_eq!(PROMTAIL.fallback.len(), 1);
        assert_eq!(
            PROMTAIL.fallback[0].eco,
            crate::tools::spec::FallbackEco::Go
        );
        assert_eq!(
            PROMTAIL.fallback[0].package,
            "github.com/grafana/loki/v3/clients/cmd/promtail"
        );
    }
}
