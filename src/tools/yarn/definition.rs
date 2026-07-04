//! Yarn — JavaScript package manager. Detected via `yarn.lock`.
//!
//! Note: Yarn 2+ (Berry) is typically vendored per-repo under
//! `.yarn/releases/` and driven by `corepack yarn`; installing the
//! standalone `yarn` (v1 / classic) is the right compat move when the
//! repo hasn't opted in to Berry.

use crate::define_tool;

define_tool!(YARN, {
    command: "yarn",
    repo: "yarnpkg/yarn",
    macos: { brew: "yarn" },
    linux: { brew: "yarn" },
    windows: { winget: "Yarn.Yarn" },
});

// Registration-shape test deleted per Maint F3 — `define_tool!`
// already type-checks the field assignments at compile time.
