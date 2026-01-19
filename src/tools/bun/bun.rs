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
    fn ensure_bun_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
