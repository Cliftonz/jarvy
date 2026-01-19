//! nmap - Network exploration and security auditing tool
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(NMAP, {
    command: "nmap",
    macos: { brew: "nmap" },
    linux: { uniform: "nmap" },
    windows: { winget: "Insecure.Nmap" },
    bsd: { pkg: "nmap" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_nmap_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
