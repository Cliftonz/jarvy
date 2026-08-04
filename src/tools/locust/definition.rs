//! Locust - scalable load-testing framework in Python
//!
//! `locust` runs distributed load tests defined as plain Python
//! locustfiles, with a built-in web UI for live stats. Python-based;
//! homebrew-core packages it with its own virtualenv on both macOS
//! and Linux, so no system Python wrangling is required.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(LOCUST, {
    command: "locust",
    macos: { brew: "locust" },
    // Linux: no distro package; homebrew-core ships arm64/x86_64
    // Linux bottles for `locust`.
    linux: { brew: "locust" },
    // No first-party winget manifest as of 2026-07; the uv fallback
    // route covers Windows via PyPI `locust` (verified 2026-08).
    fallback: { uv: "locust" },
    category: "testing",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locust_registration_shape() {
        assert_eq!(LOCUST.command, "locust");
        assert_eq!(LOCUST.category, Some("testing"));
        let mac = LOCUST.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("locust"));
        let linux = LOCUST.linux.expect("must support Linux");
        assert_eq!(linux.brew, Some("locust"));
        assert!(LOCUST.windows.is_none(), "no first-party winget manifest");
        assert_eq!(LOCUST.fallback.len(), 1);
        assert_eq!(LOCUST.fallback[0].eco, crate::tools::spec::FallbackEco::Uv);
        assert_eq!(LOCUST.fallback[0].package, "locust");
    }
}
