//! nmap - Network exploration and security auditing tool
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(NMAP, {
    command: "nmap",
    repo: "nmap/nmap",
    macos: { brew: "nmap" },
    linux: { uniform: "nmap" },
    windows: { winget: "Insecure.Nmap" },
    bsd: { pkg: "nmap" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nmap_registration_shape() {
        assert_eq!(NMAP.command, "nmap");
        let mac = NMAP.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("nmap"));
        let win = NMAP.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Insecure.Nmap"));
    }
}
