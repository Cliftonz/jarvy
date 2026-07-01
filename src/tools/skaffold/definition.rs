//! Skaffold — Google's inner-dev-loop CLI for Kubernetes: watch,
//! rebuild, redeploy on every source change.
//!
//! Homepage: <https://skaffold.dev>. Marker: `skaffold.yaml` at the
//! repo root. Linuxbrew ships a bottle (verified 2026-07), so we
//! route Linux through brew rather than picking a distro-specific
//! path — Skaffold has no apt / dnf / pacman package as of writing.

use crate::define_tool;

define_tool!(SKAFFOLD, {
    command: "skaffold",
    macos: { brew: "skaffold" },
    linux: { brew: "skaffold" },
    windows: { winget: "Google.ContainerTools.Skaffold" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skaffold_registration_shape() {
        assert_eq!(SKAFFOLD.command, "skaffold");
        assert_eq!(SKAFFOLD.macos.unwrap().brew, Some("skaffold"));
        assert_eq!(
            SKAFFOLD.windows.unwrap().winget,
            Some("Google.ContainerTools.Skaffold")
        );
    }
}
