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
    // No first-party winget manifest as of 2026-08; install from
    // https://docs.kubelinter.io/#/using-kubelinter?id=installing-kubelinter
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
    }
}
