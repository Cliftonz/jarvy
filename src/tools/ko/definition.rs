//! ko - Build and deploy Go applications to Kubernetes
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(KO, {
    command: "ko",
    repo: "ko-build/ko",
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
    fn ko_registration_shape() {
        assert_eq!(KO.command, "ko");
        let mac = KO.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("ko"));
        let win = KO.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("ko-build.ko"));
    }
}
