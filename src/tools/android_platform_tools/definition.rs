//! Android SDK Platform-Tools - adb and fastboot device tooling
//!
//! The registry probes `adb`, the primary development command supplied by the
//! platform-tools bundle. Package names follow each operating system's native
//! package rather than downloading an unpinned SDK archive.

use crate::define_tool;

define_tool!(ANDROID_PLATFORM_TOOLS, {
    command: "adb",
    macos: { cask: "android-platform-tools" },
    linux: { apt: "adb", dnf: "android-tools", pacman: "android-tools", apk: "android-tools" },
    windows: { winget: "Google.PlatformTools" },
    category: "mobile",
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_platform_tools_registration_shape() {
        assert_eq!(ANDROID_PLATFORM_TOOLS.command, "adb");
        assert_eq!(ANDROID_PLATFORM_TOOLS.category, Some("mobile"));

        let mac = ANDROID_PLATFORM_TOOLS.macos.expect("must support macOS");
        assert_eq!(mac.cask, Some("android-platform-tools"));

        let linux = ANDROID_PLATFORM_TOOLS.linux.expect("must support Linux");
        assert_eq!(linux.apt, Some("adb"));
        assert_eq!(linux.dnf, Some("android-tools"));
        assert_eq!(linux.pacman, Some("android-tools"));
        assert_eq!(linux.apk, Some("android-tools"));

        let windows = ANDROID_PLATFORM_TOOLS
            .windows
            .expect("must support Windows");
        assert_eq!(windows.winget, Some("Google.PlatformTools"));
    }
}
