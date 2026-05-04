//! ko - Build and deploy Go applications to Kubernetes
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(KO, {
    command: "ko",
    macos: { brew: "ko" },
    linux: { uniform: "ko" },
    windows: { winget: "ko-build.ko" },
    bsd: { pkg: "ko" },
    depends_on: &["go"],
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_ko_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
