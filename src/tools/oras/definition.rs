//! oras - OCI Registry As Storage
//!
//! ORAS pushes and pulls OCI artifacts to and from OCI registries.
//! It enables using container registries for arbitrary content distribution.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ORAS, {
    command: "oras",
    repo: "oras-project/oras",
    macos: { brew: "oras" },
    linux: { uniform: "oras" },
    windows: { winget: "oras-project.oras" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oras_registration_shape() {
        assert_eq!(ORAS.command, "oras");
        let mac = ORAS.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("oras"));
        let win = ORAS.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("oras-project.oras"));
    }
}
