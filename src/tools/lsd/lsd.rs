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
    fn ensure_lsd_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
