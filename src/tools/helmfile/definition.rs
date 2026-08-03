//! helmfile — declarative Helm release management.
//!
//! Bundles many `helm install/upgrade/uninstall` invocations behind one
//! `helmfile.yaml` so a stack of releases can be applied atomically. Depends
//! on `helm` (which itself depends on `kubectl`), so declaring `helm` here
//! seeds the whole chain through the topo sort in the setup phase.

use crate::define_tool;

define_tool!(HELMFILE, {
    command: "helmfile",
    macos: { brew: "helmfile" },
    linux: { brew: "helmfile" },
    // No first-party winget manifest as of 2026-08; install from
    // https://helmfile.readthedocs.io/en/stable/#installation
    depends_on: &["helm"],
    category: "devops",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helmfile_registration_shape() {
        assert_eq!(HELMFILE.command, "helmfile");
        assert_eq!(HELMFILE.macos.expect("macOS").brew, Some("helmfile"));
        assert_eq!(HELMFILE.linux.expect("Linux").brew, Some("helmfile"));
        assert!(HELMFILE.windows.is_none(), "no first-party winget yet");
        assert!(
            HELMFILE
                .depends_on
                .expect("declares deps")
                .contains(&"helm")
        );
    }
}
