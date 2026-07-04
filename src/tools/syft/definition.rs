//! syft - SBOM generator for containers
//!
//! Syft generates a Software Bill of Materials (SBOM) from container images
//! and filesystems.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(SYFT, {
    command: "syft",
    repo: "anchore/syft",
    macos: { brew: "syft" },
    linux: { brew: "syft", apk: "syft" },
    windows: { choco: "syft" },
    bsd: { pkg: "syft" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syft_registration_shape() {
        assert_eq!(SYFT.command, "syft");
        let mac = SYFT.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("syft"));
    }
}
