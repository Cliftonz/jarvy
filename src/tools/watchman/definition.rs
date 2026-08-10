//! Watchman - recursive file watching service
//!
//! Watchman is commonly used by React Native's Metro development workflow.
//! Meta recommends Homebrew for macOS and Linux and documents the community
//! Chocolatey package for Windows.

use crate::define_tool;

define_tool!(WATCHMAN, {
    command: "watchman",
    macos: { brew: "watchman" },
    linux: { brew: "watchman" },
    windows: { choco: "watchman" },
    category: "mobile",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watchman_registration_shape() {
        assert_eq!(WATCHMAN.command, "watchman");
        assert_eq!(WATCHMAN.category, Some("mobile"));

        let mac = WATCHMAN.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("watchman"));

        let linux = WATCHMAN.linux.expect("must support Linux");
        assert_eq!(linux.brew, Some("watchman"));

        let windows = WATCHMAN.windows.expect("must support Windows");
        assert_eq!(windows.choco, Some("watchman"));
    }
}
