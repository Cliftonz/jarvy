//! earthly - build automation for the container era
//!
//! Earthly is a build automation tool for the container era. It allows
//! you to execute all your builds in containers, making them
//! self-contained, reproducible, portable, and parallel.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(EARTHLY, {
    command: "earthly",
    macos: { brew: "earthly/earthly/earthly" },
    linux: { uniform: "earthly" },
    windows: { winget: "Earthly.Earthly" },
    bsd: { pkg: "earthly" },
    depends_on_one_of: &["docker", "podman"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_earthly_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
