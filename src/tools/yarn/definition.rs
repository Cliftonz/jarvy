//! Yarn — JavaScript package manager. Detected via `yarn.lock`.
//!
//! Note: Yarn 2+ (Berry) is typically vendored per-repo under
//! `.yarn/releases/` and driven by `corepack yarn`; installing the
//! standalone `yarn` (v1 / classic) is the right compat move when the
//! repo hasn't opted in to Berry.

use crate::define_tool;

define_tool!(YARN, {
    command: "yarn",
    macos: { brew: "yarn" },
    linux: { brew: "yarn" },
    windows: { winget: "Yarn.Yarn" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yarn_registration_shape() {
        assert_eq!(YARN.command, "yarn");
        assert_eq!(YARN.macos.unwrap().brew, Some("yarn"));
        assert_eq!(YARN.windows.unwrap().winget, Some("Yarn.Yarn"));
    }
}
