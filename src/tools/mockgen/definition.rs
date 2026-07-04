//! mockgen - Go mock generator for interfaces
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(MOCKGEN, {
    command: "mockgen",
    repo: "uber-go/mock",
    macos: { brew: "mockery" },
    linux: { uniform: "mockery" },
    windows: { winget: "vektra.mockery" },
    bsd: { pkg: "mockery" },
    depends_on: &["go"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mockgen_registration_shape() {
        assert_eq!(MOCKGEN.command, "mockgen");
        let mac = MOCKGEN.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("mockery"));
        let win = MOCKGEN.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("vektra.mockery"));
    }
}
