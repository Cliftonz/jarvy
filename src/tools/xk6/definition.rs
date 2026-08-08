//! xk6 - k6 extension builder
//!
//! xk6 builds a custom `k6` binary bundled with third-party extensions
//! (JS modules or output writers). Uses the vanity import
//! `go.k6.io/xk6` rather than the GitHub path.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(XK6, {
    command: "xk6",
    // No first-party brew, apt/dnf, or winget package as of 2026-08.
    // The go fallback covers every platform (canonical vanity path).
    fallback: { go: "go.k6.io/xk6/cmd/xk6" },
    category: "testing",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xk6_registration_shape() {
        assert_eq!(XK6.command, "xk6");
        assert_eq!(XK6.category, Some("testing"));
        assert!(XK6.macos.is_none());
        assert!(XK6.linux.is_none());
        assert!(XK6.windows.is_none());
        assert_eq!(XK6.fallback.len(), 1);
        assert_eq!(XK6.fallback[0].eco, crate::tools::spec::FallbackEco::Go);
        assert_eq!(XK6.fallback[0].package, "go.k6.io/xk6/cmd/xk6");
    }
}
