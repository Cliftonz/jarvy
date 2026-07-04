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
    repo: "GoogleContainerTools/skaffold",
    macos: { brew: "skaffold" },
    linux: { brew: "skaffold" },
    windows: { winget: "Google.ContainerTools.Skaffold" },
});

// Registration-shape test deleted per Maint F3 — `define_tool!`
// already type-checks the field assignments at compile time.
