//! logcli - Grafana Loki query CLI
//!
//! logcli issues LogQL queries against a Loki (or Grafana Cloud Logs)
//! endpoint from the terminal. Complementary to promtail / alloy on the
//! shipping side.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(LOGCLI, {
    command: "logcli",
    // No first-party brew, apt/dnf, or winget package as of 2026-08.
    // The go fallback covers every platform.
    fallback: { go: "github.com/grafana/loki/v3/cmd/logcli" },
    category: "observability",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logcli_registration_shape() {
        assert_eq!(LOGCLI.command, "logcli");
        assert_eq!(LOGCLI.category, Some("observability"));
        assert!(LOGCLI.macos.is_none());
        assert!(LOGCLI.linux.is_none());
        assert!(LOGCLI.windows.is_none());
        assert_eq!(LOGCLI.fallback.len(), 1);
        assert_eq!(LOGCLI.fallback[0].eco, crate::tools::spec::FallbackEco::Go);
        assert_eq!(
            LOGCLI.fallback[0].package,
            "github.com/grafana/loki/v3/cmd/logcli"
        );
    }
}
