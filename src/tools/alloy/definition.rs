//! alloy - Grafana Alloy OpenTelemetry collector
//!
//! Alloy is Grafana's OpenTelemetry-compatible collector agent (the
//! successor to Grafana Agent). Ships metrics, logs, traces, and
//! profiles to any OTLP-compatible backend.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ALLOY, {
    command: "alloy",
    macos: { brew: "grafana/grafana/alloy" },
    // Linux: install via Grafana's apt/rpm repo
    // (https://grafana.com/docs/alloy/latest/set-up/install/linux/).
    // No first-party distro package; go install is not a viable fallback
    // because the build requires generate-ui + beyla pre-steps.
    windows: { winget: "GrafanaLabs.Alloy" },
    category: "observability",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloy_registration_shape() {
        assert_eq!(ALLOY.command, "alloy");
        assert_eq!(ALLOY.category, Some("observability"));
        let mac = ALLOY.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("grafana/grafana/alloy"));
        let win = ALLOY.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("GrafanaLabs.Alloy"));
        assert!(ALLOY.linux.is_none(), "Linux uses Grafana apt/rpm repo");
        assert!(
            ALLOY.fallback.is_empty(),
            "no go fallback (build needs generate-ui)"
        );
    }
}
