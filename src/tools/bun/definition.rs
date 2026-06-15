//! bun - incredibly fast JavaScript runtime
//!
//! Bun is an all-in-one JavaScript runtime & toolkit designed for speed.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(BUN, {
    command: "bun",
    macos: { brew: "oven-sh/bun/bun" },
    linux: { brew: "oven-sh/bun/bun" },
    windows: { winget: "Oven-sh.Bun" },
    bsd: { pkg: "bun" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bun_registration_shape() {
        assert_eq!(BUN.command, "bun");
        let mac = BUN.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("oven-sh/bun/bun"));
        let win = BUN.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Oven-sh.Bun"));
    }
}
