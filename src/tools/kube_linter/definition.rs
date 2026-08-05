//! kube-linter — static analysis for Kubernetes YAML and Helm charts.
//!
//! Complementary to `kubeconform`: kubeconform validates SCHEMA
//! conformance, kube-linter detects insecure or problematic WORKLOAD
//! configuration (missing readiness probes, privileged containers,
//! host-path mounts, etc.). Runs offline, no cluster access needed.
//!
//! Tool name uses the dash spelling everywhere (matches the upstream
//! binary and the brew formula); the `-` ↔ `_` alias in the tool
//! registry lets `jarvy.toml` users write either form.

use crate::define_tool;

define_tool!(KUBE_LINTER, {
    command: "kube-linter",
    macos: { brew: "kube-linter" },
    linux: { brew: "kube-linter" },
    // No first-party winget manifest as of 2026-08; the go fallback
    // route covers Windows — upstream README documents exactly this
    // go install path (verified 2026-08).
    fallback: { go: "golang.stackrox.io/kube-linter/cmd/kube-linter" },
    category: "devops",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kube_linter_registration_shape() {
        assert_eq!(KUBE_LINTER.command, "kube-linter");
        assert_eq!(KUBE_LINTER.macos.expect("macOS").brew, Some("kube-linter"));
        assert_eq!(KUBE_LINTER.linux.expect("Linux").brew, Some("kube-linter"));
        assert!(KUBE_LINTER.windows.is_none(), "no first-party winget yet");
        assert_eq!(KUBE_LINTER.fallback.len(), 1);
        assert_eq!(
            KUBE_LINTER.fallback[0].eco,
            crate::tools::spec::FallbackEco::Go
        );
        assert_eq!(
            KUBE_LINTER.fallback[0].package,
            "golang.stackrox.io/kube-linter/cmd/kube-linter"
        );
    }
}
