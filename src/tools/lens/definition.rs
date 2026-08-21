//! Lens - Kubernetes IDE (Mirantis)
//!
//! Standalone Kubernetes IDE for macOS, Windows, and Linux.
//! Homepage: https://k8slens.dev
//!
//! No first-party Linux package-manager entry as of 2026-08; users install
//! the .deb / .rpm / Snap / Flatpak from upstream. The open-source community
//! fork lives in `src/tools/freelens/`.

use crate::define_tool;

define_tool!(LENS, {
    command: "lens",
    macos: { cask: "lens" },
    windows: { winget: "Mirantis.Lens" },
});
