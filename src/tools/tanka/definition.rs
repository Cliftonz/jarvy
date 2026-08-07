//! tanka - Grafana Tanka (Jsonnet-based Kubernetes config)
//!
//! Tanka declares Kubernetes resources in Jsonnet. Binary on PATH is
//! `tk`, config key stays `tanka` per Jarvy convention.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(TANKA, {
    command: "tk",
    macos: { brew: "tanka" },
    // No first-party apt/dnf or winget package as of 2026-08. Grafana's
    // install docs point Linux users at the release tarball or `go
    // install`; Windows users to `go install`. The go fallback covers
    // both.
    fallback: { go: "github.com/grafana/tanka/cmd/tk" },
    category: "kubernetes",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tanka_registration_shape() {
        assert_eq!(TANKA.command, "tk");
        assert_eq!(TANKA.category, Some("kubernetes"));
        let mac = TANKA.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("tanka"));
        assert!(TANKA.linux.is_none());
        assert!(TANKA.windows.is_none());
        assert_eq!(TANKA.fallback.len(), 1);
        assert_eq!(TANKA.fallback[0].eco, crate::tools::spec::FallbackEco::Go);
        assert_eq!(TANKA.fallback[0].package, "github.com/grafana/tanka/cmd/tk");
    }
}
