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
    macos: { brew: "cmake" },
    linux: { uniform: "cmake" },
    windows: { winget: "Kitware.CMake" },
    bsd: { pkg: "cmake" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmake_registration_shape() {
        assert_eq!(CMAKE.command, "cmake");
        assert_eq!(CMAKE.macos.unwrap().brew, Some("cmake"));
        assert_eq!(CMAKE.windows.unwrap().winget, Some("Kitware.CMake"));
    }
}
