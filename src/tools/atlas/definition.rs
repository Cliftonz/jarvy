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
    fn atlas_registration_shape() {
        assert_eq!(ATLAS.command, "atlas");
        let mac = ATLAS.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("ariga/tap/atlas"));
        let win = ATLAS.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Ariga.Atlas"));
    }
}
