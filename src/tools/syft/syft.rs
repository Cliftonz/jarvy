//! syft - SBOM generator for containers
//!
//! Syft generates a Software Bill of Materials (SBOM) from container images
//! and filesystems.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(SYFT, {
    command: "syft",
    macos: { brew: "syft" },
    linux: { brew: "syft", apk: "syft" },
    windows: { choco: "syft" },
    bsd: { pkg: "syft" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_syft_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
