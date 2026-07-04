//! lnav - Log file navigator with syntax highlighting
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: No Windows support available.

use crate::define_tool;

define_tool!(LNAV, {
    command: "lnav",
    repo: "tstack/lnav",
    macos: { brew: "lnav" },
    linux: { uniform: "lnav" },
    bsd: { pkg: "lnav" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lnav_registration_shape() {
        assert_eq!(LNAV.command, "lnav");
        let mac = LNAV.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("lnav"));
    }
}
