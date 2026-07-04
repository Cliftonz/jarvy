//! cosign - container signing and verification
//!
//! Cosign is a tool for signing and verifying container images.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(COSIGN, {
    command: "cosign",
    repo: "sigstore/cosign",
    macos: { brew: "cosign" },
    linux: { brew: "cosign", apk: "cosign" },
    windows: { winget: "sigstore.cosign" },
    bsd: { pkg: "cosign" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosign_registration_shape() {
        assert_eq!(COSIGN.command, "cosign");
        let mac = COSIGN.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("cosign"));
        let win = COSIGN.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("sigstore.cosign"));
    }
}
