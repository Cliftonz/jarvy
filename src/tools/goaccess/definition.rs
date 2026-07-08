//! GoAccess - real-time web log analyzer
//!
//! `goaccess` parses Apache/Nginx/CloudFront/etc. access logs and
//! renders live dashboards in the terminal or as standalone HTML.
//! Packaged under the same name everywhere: Debian/Ubuntu, Fedora,
//! Arch, openSUSE, Alpine (main), and homebrew-core.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GOACCESS, {
    command: "goaccess",
    macos: { brew: "goaccess" },
    linux: { uniform: "goaccess" },
    // Windows is not supported upstream (POSIX-only; WSL works) —
    // see https://goaccess.io/download.
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goaccess_registration_shape() {
        assert_eq!(GOACCESS.command, "goaccess");
        let mac = GOACCESS.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("goaccess"));
        let linux = GOACCESS.linux.expect("must support Linux");
        assert_eq!(linux.apt, Some("goaccess"));
        assert!(GOACCESS.windows.is_none(), "upstream is POSIX-only");
    }
}
