//! oras - OCI Registry As Storage
//!
//! ORAS pushes and pulls OCI artifacts to and from OCI registries.
//! It enables using container registries for arbitrary content distribution.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ORAS, {
    command: "oras",
    macos: { brew: "oras" },
    linux: { uniform: "oras" },
    windows: { winget: "oras-project.oras" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_oras_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
