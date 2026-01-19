//! sops - Secrets OPerationS - manage encrypted files
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(SOPS, {
    command: "sops",
    macos: { brew: "sops" },
    linux: { brew: "sops" },
    windows: { winget: "Mozilla.sops" },
    bsd: { pkg: "sops" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_sops_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
