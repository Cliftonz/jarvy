//! mtr - Network diagnostic tool (traceroute + ping)
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Note: No Windows support available.

use crate::define_tool;

define_tool!(MTR, {
    command: "mtr",
    repo: "traviscross/mtr",
    macos: { brew: "mtr" },
    linux: { uniform: "mtr" },
    bsd: { pkg: "mtr" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mtr_registration_shape() {
        assert_eq!(MTR.command, "mtr");
        let mac = MTR.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("mtr"));
    }
}
