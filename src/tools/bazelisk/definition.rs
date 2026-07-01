//! Bazelisk — user-friendly launcher for Bazel that respects
//! `.bazeliskrc` / `.bazelversion` per project. Ships as the
//! recommended install path over the raw `bazel` binary because
//! Bazel projects pin specific Bazel versions and Bazelisk swaps
//! automatically.
//!
//! Homepage: <https://github.com/bazelbuild/bazelisk>. Marker: any of
//! `WORKSPACE`, `WORKSPACE.bazel`, `MODULE.bazel`, `BUILD.bazel`, or
//! `.bazelrc` at the repo root (see the `bazelisk` rule in
//! `discover/rules.rs`).

use crate::define_tool;

define_tool!(BAZELISK, {
    command: "bazelisk",
    macos: { brew: "bazelisk" },
    linux: { brew: "bazelisk" },
    windows: { winget: "Bazel.Bazelisk" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bazelisk_registration_shape() {
        assert_eq!(BAZELISK.command, "bazelisk");
        assert_eq!(BAZELISK.macos.unwrap().brew, Some("bazelisk"));
        assert_eq!(BAZELISK.windows.unwrap().winget, Some("Bazel.Bazelisk"));
    }
}
