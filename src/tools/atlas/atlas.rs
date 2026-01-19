//! atlas - Ariga Atlas database schema management
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ATLAS, {
    command: "atlas",
    macos: { brew: "ariga/tap/atlas" },
    linux: { uniform: "atlas" },
    windows: { winget: "Ariga.Atlas" },
    bsd: { pkg: "atlas" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_atlas_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
