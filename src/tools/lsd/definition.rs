//! lsd - the next gen ls command
//!
//! LSd (LSDeluxe) is a rewrite of GNU ls with lots of added features like
//! colors, icons, tree-view, and more formatting options.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(LSD, {
    command: "lsd",
    macos: { brew: "lsd" },
    linux: { uniform: "lsd" },
    windows: { winget: "lsd-rs.lsd" },
    bsd: { pkg: "lsd" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsd_registration_shape() {
        assert_eq!(LSD.command, "lsd");
        let mac = LSD.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("lsd"));
        let win = LSD.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("lsd-rs.lsd"));
    }
}
