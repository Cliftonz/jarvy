//! duf - Disk usage/free utility with colors
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(DUF, {
    command: "duf",
    repo: "muesli/duf",
    macos: { brew: "duf" },
    linux: { uniform: "duf" },
    windows: { winget: "muesli.duf" },
    bsd: { pkg: "duf" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duf_registration_shape() {
        assert_eq!(DUF.command, "duf");
        let mac = DUF.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("duf"));
        let win = DUF.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("muesli.duf"));
    }
}
