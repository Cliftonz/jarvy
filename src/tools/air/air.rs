//! air - live reload for Go applications
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(AIR, {
    command: "air",
    macos: { brew: "air" },
    linux: { uniform: "air" },
    windows: { winget: "cosmtrek.air" },
    bsd: { pkg: "air" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_air_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
