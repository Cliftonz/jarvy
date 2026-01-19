//! cosign - container signing and verification
//!
//! Cosign is a tool for signing and verifying container images.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(COSIGN, {
    command: "cosign",
    macos: { brew: "cosign" },
    linux: { brew: "cosign", apk: "cosign" },
    windows: { winget: "sigstore.cosign" },
    bsd: { pkg: "cosign" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_cosign_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
