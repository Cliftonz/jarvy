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
    repo: "bazelbuild/bazelisk",
    macos: { brew: "bazelisk" },
    linux: { brew: "bazelisk" },
    windows: { winget: "Bazel.Bazelisk" },
});

// Per the maintainability audit: a `bazelisk_registration_shape` test
// that re-asserts `TOOL.command == "bazelisk"`, `TOOL.macos.brew ==
// Some("bazelisk")`, etc. is a macro-tautology — the `define_tool!`
// invocation above already type-checks those field assignments at
// compile time. Deleted rather than extracted into a shared helper
// (Maint F3): if the macro expansion regresses, the whole build
// fails at type-check, not at test-time.
