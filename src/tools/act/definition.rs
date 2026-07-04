//! act - run GitHub Actions locally
//!
//! act lets you run your GitHub Actions locally using Docker.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ACT, {
    command: "act",
    repo: "nektos/act",
    macos: { brew: "act" },
    linux: { brew: "act" },
    windows: { winget: "nektos.act", choco: "act-cli" },
    bsd: { pkg: "act" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn act_registration_shape() {
        assert_eq!(ACT.command, "act");
        let mac = ACT.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("act"));
        let win = ACT.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("nektos.act"));
    }
}
