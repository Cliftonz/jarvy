//! act - run GitHub Actions locally
//!
//! act lets you run your GitHub Actions locally using Docker.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(ACT, {
    command: "act",
    macos: { brew: "act" },
    linux: { brew: "act" },
    windows: { winget: "nektos.act", choco: "act-cli" },
    bsd: { pkg: "act" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_act_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
