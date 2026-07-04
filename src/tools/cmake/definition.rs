//! CMake — cross-platform build system generator.
//!
//! Homepage: <https://cmake.org>. Marker: `CMakeLists.txt` at the repo
//! root (see the `cmake` rule in `discover/rules.rs`). Standard
//! packaging on every mainstream package manager — uniform Linux name
//! across apt/dnf/pacman/apk, first-party Homebrew formula, verified
//! `Kitware.CMake` winget manifest.

use crate::define_tool;

define_tool!(CMAKE, {
    command: "cmake",
    repo: "Kitware/CMake",
    macos: { brew: "cmake" },
    linux: { uniform: "cmake" },
    windows: { winget: "Kitware.CMake" },
    bsd: { pkg: "cmake" },
});

// Registration-shape test deleted per Maint F3 — `define_tool!`
// already type-checks the field assignments at compile time.
